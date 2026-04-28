//! Cognitum Cog: RuView DensePose
//!
//! Full RuView integration stub. Connects to ESP32 CSI stream and
//! runs Hampel filter + phase sanitization, outputting a 17-keypoint
//! skeleton proxy from processed CSI features.
//!
//! Keypoints (COCO format):
//!   0=nose, 1=left_eye, 2=right_eye, 3=left_ear, 4=right_ear,
//!   5=left_shoulder, 6=right_shoulder, 7=left_elbow, 8=right_elbow,
//!   9=left_wrist, 10=right_wrist, 11=left_hip, 12=right_hip,
//!   13=left_knee, 14=right_knee, 15=left_ankle, 16=right_ankle
//!
//! Usage:
//!   cog-ruview-densepose [--once] [--interval 1] [--source SOURCE]
//!
//! Sources (--source, ADR-091):
//!   auto                    (default — try UDP :5005 first, fall back to seed-stream)
//!   seed-stream             agent's /api/v1/sensor/stream over loopback
//!   esp32-uart=<path>       ESP32 serial port; build with --features esp32-uart
//!   esp32-udp=<host:port>   bind UDP, parse ADR-069 0xC5110003 packets;
//!                           build with --features esp32-udp
//!
//! ADR-091: cogs are self-contained. The cog brings its own sensor
//! source rather than depending on the agent's sensor/stream aggregator.

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::time::{Duration, Instant};

const UDP_PORT: u16 = 5005;
const HAMPEL_WINDOW: usize = 7;
const HAMPEL_THRESHOLD: f64 = 3.0;
const NUM_KEYPOINTS: usize = 17;

/// Hampel filter: replace outliers with median in a sliding window
fn hampel_filter(data: &mut Vec<f64>) {
    if data.len() < HAMPEL_WINDOW { return; }
    let half = HAMPEL_WINDOW / 2;
    let original = data.clone();

    for i in half..data.len().saturating_sub(half) {
        let start = i - half;
        let end = (i + half + 1).min(original.len());
        let mut window: Vec<f64> = original[start..end].to_vec();
        window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = window[window.len() / 2];
        // MAD (Median Absolute Deviation)
        let mut deviations: Vec<f64> = window.iter().map(|v| (v - median).abs()).collect();
        deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mad = deviations[deviations.len() / 2] * 1.4826; // Scale to std dev

        if mad > 1e-10 && (original[i] - median).abs() / mad > HAMPEL_THRESHOLD {
            data[i] = median; // Replace outlier
        }
    }
}

/// Phase sanitization: unwrap phase jumps > pi
fn phase_sanitize(data: &mut [f64]) {
    for i in 1..data.len() {
        let mut diff = data[i] - data[i - 1];
        while diff > std::f64::consts::PI { diff -= 2.0 * std::f64::consts::PI; }
        while diff < -std::f64::consts::PI { diff += 2.0 * std::f64::consts::PI; }
        data[i] = data[i - 1] + diff;
    }
}

/// Map processed CSI features to 17 keypoints.
/// This is a proxy mapping — real DensePose would use a trained neural net.
/// We use signal decomposition to estimate body part positions.
#[derive(serde::Serialize, Clone)]
struct Keypoint {
    name: &'static str,
    x: f64,
    y: f64,
    confidence: f64,
}

const KEYPOINT_NAMES: [&str; 17] = [
    "nose", "left_eye", "right_eye", "left_ear", "right_ear",
    "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
    "left_wrist", "right_wrist", "left_hip", "right_hip",
    "left_knee", "right_knee", "left_ankle", "right_ankle",
];

fn csi_to_keypoints(features: &[f64]) -> Vec<Keypoint> {
    let n = features.len();
    if n == 0 {
        return KEYPOINT_NAMES.iter().map(|name| Keypoint {
            name, x: 0.0, y: 0.0, confidence: 0.0,
        }).collect();
    }

    // Normalize features to [0, 1]
    let min_f = features.iter().cloned().fold(f64::MAX, f64::min);
    let max_f = features.iter().cloned().fold(f64::MIN, f64::max);
    let range = (max_f - min_f).max(1e-10);
    let norm: Vec<f64> = features.iter().map(|&v| (v - min_f) / range).collect();

    // Map feature indices to keypoints using spatial decomposition
    // Upper body uses higher-frequency components, lower body uses lower
    KEYPOINT_NAMES.iter().enumerate().map(|(kp_idx, &name)| {
        // Each keypoint gets a slice of the feature space
        let feat_start = (kp_idx * n) / NUM_KEYPOINTS;
        let feat_end = ((kp_idx + 1) * n) / NUM_KEYPOINTS;
        let slice = &norm[feat_start..feat_end.min(n)];

        if slice.is_empty() {
            return Keypoint { name, x: 0.0, y: 0.0, confidence: 0.0 };
        }

        // X from even-indexed features, Y from odd-indexed
        let x: f64 = slice.iter().step_by(2).sum::<f64>()
            / (slice.len() / 2 + 1).max(1) as f64;
        let y: f64 = slice.iter().skip(1).step_by(2).sum::<f64>()
            / (slice.len() / 2).max(1) as f64;

        // Confidence from signal energy in this band
        let energy: f64 = slice.iter().map(|v| v * v).sum::<f64>() / slice.len() as f64;
        let confidence = energy.sqrt().min(1.0);

        Keypoint { name, x, y, confidence }
    }).collect()
}

struct DensePoseState {
    channel_buffers: Vec<Vec<f64>>,
    buffer_max: usize,
}

impl DensePoseState {
    fn new() -> Self {
        Self {
            channel_buffers: Vec::new(),
            buffer_max: 64,
        }
    }
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match conn.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => { buf.extend_from_slice(&tmp[..n]); if buf.len() > 262144 { break; } }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) if !buf.is_empty() => break, // Got data before error — use it
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn try_udp_csi() -> Vec<f64> {
    let socket = match UdpSocket::bind(format!("0.0.0.0:{UDP_PORT}")) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    socket.set_read_timeout(Some(Duration::from_millis(200))).ok();

    let mut buf = [0u8; 4096];
    match socket.recv_from(&mut buf) {
        Ok((len, _)) => {
            // Try JSON
            if let Ok(frame) = serde_json::from_slice::<serde_json::Value>(&buf[..len]) {
                if let Some(subs) = frame.get("subcarriers").and_then(|v| v.as_array()) {
                    return subs.iter().filter_map(|v| v.as_f64()).collect();
                }
            }
            // Try raw binary: skip 10-byte header
            if len > 10 {
                let n_sub = u16::from_le_bytes([buf[8], buf[9]]) as usize;
                let mut vals = Vec::with_capacity(n_sub);
                for i in 0..n_sub {
                    let off = 10 + i * 4;
                    if off + 4 <= len {
                        vals.push(f32::from_le_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3]]) as f64);
                    }
                }
                return vals;
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

fn store_vector(vec: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec.to_vec()]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

#[derive(serde::Serialize)]
struct DensePoseReport {
    keypoints: Vec<Keypoint>,
    num_keypoints: usize,
    avg_confidence: f64,
    source: String,   // "sensor_api" or "udp_csi"
    raw_features: usize,
    hampel_corrections: usize,
    timestamp: u64,
}

// ── Sensor sources (ADR-091) ─────────────────────────────────────────

enum Source {
    /// Default — try UDP :5005 first, fall back to seed-stream (v1.0.0 behavior).
    Auto,
    /// Agent's /api/v1/sensor/stream only.
    SeedStream,
    /// ESP32 serial-port reader; gated behind --features esp32-uart.
    Esp32Uart(String),
    /// Explicit UDP listener with configurable bind addr; gated behind --features esp32-udp.
    Esp32Udp(String),
}

fn parse_source_arg(spec: &str) -> Result<Source, String> {
    if spec == "auto" || spec.is_empty() { return Ok(Source::Auto); }
    if spec == "seed-stream" { return Ok(Source::SeedStream); }
    if let Some(p) = spec.strip_prefix("esp32-uart=") {
        if p.is_empty() { return Err("esp32-uart= requires a path (e.g. COM8 or /dev/ttyACM0)".into()); }
        return Ok(Source::Esp32Uart(p.to_string()));
    }
    if let Some(a) = spec.strip_prefix("esp32-udp=") {
        if a.is_empty() { return Err("esp32-udp= requires bind_host:port (e.g. 0.0.0.0:5006)".into()); }
        return Ok(Source::Esp32Udp(a.to_string()));
    }
    Err(format!("unknown source '{}'; expected: auto | seed-stream | esp32-uart=PATH | esp32-udp=HOST:PORT", spec))
}

#[cfg(feature = "esp32-uart")]
fn fetch_from_esp32_uart(path: &str, window_ms: u64, max_samples: usize) -> Result<Vec<f64>, String> {
    let mut port = serialport::new(path, 115_200)
        .timeout(Duration::from_millis(200))
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::None)
        .open()
        .map_err(|e| format!("open {}: {}", path, e))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 1024];
    let deadline = Instant::now() + Duration::from_millis(window_ms);
    while Instant::now() < deadline && buf.len() < 65536 {
        match port.read(&mut tmp) {
            Ok(0) => continue,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(format!("uart read: {}", e)),
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let amps: Vec<f64> = text.split_whitespace()
        .filter_map(|t| t.strip_prefix("rssi="))
        .filter_map(|s| s.trim_end_matches(',').parse::<f64>().ok())
        .map(|dbm| ((dbm + 65.0) / 35.0).clamp(-1.0, 1.0))
        .take(max_samples).collect();
    if amps.is_empty() {
        return Err(format!("no rssi=N tokens in {}B from {} — esp32-csi-node firmware running?", buf.len(), path));
    }
    Ok(amps)
}

#[cfg(not(feature = "esp32-uart"))]
fn fetch_from_esp32_uart(_p: &str, _w: u64, _m: usize) -> Result<Vec<f64>, String> {
    Err("--source esp32-uart not enabled in this build (rebuild with --features esp32-uart)".into())
}

#[cfg(feature = "esp32-udp")]
fn fetch_from_esp32_udp(addr: &str, window_ms: u64, max_samples: usize) -> Result<Vec<f64>, String> {
    const MAGIC_FEATURES: u32 = 0xC511_0003;
    const FEATURE_PKT_SIZE: usize = 48;
    let socket = UdpSocket::bind(addr).map_err(|e| format!("bind {}: {}", addr, e))?;
    socket.set_read_timeout(Some(Duration::from_millis(window_ms.min(2000))))
        .map_err(|e| format!("set timeout: {}", e))?;
    let mut amps: Vec<f64> = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(window_ms);
    let mut pkt = [0u8; 256];
    while Instant::now() < deadline && amps.len() < max_samples {
        match socket.recv_from(&mut pkt) {
            Ok((n, _)) if n >= FEATURE_PKT_SIZE => {
                let magic = u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]);
                if magic != MAGIC_FEATURES { continue; }
                for i in 0..8 {
                    let off = 16 + i * 4;
                    if off + 4 > n { break; }
                    let f = f32::from_le_bytes([pkt[off], pkt[off+1], pkt[off+2], pkt[off+3]]);
                    if f.is_finite() { amps.push((f as f64).clamp(-1.0, 1.0)); }
                }
            }
            Ok(_) => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("udp recv: {}", e)),
        }
    }
    if amps.is_empty() {
        return Err(format!("no ADR-069 packets on {} within {}ms", addr, window_ms));
    }
    Ok(amps)
}

#[cfg(not(feature = "esp32-udp"))]
fn fetch_from_esp32_udp(_a: &str, _w: u64, _m: usize) -> Result<Vec<f64>, String> {
    Err("--source esp32-udp not enabled in this build (rebuild with --features esp32-udp)".into())
}

fn run_once(state: &mut DensePoseState, source: &Source, window_ms: u64) -> Result<DensePoseReport, String> {
    let (mut features, src_tag) = match source {
        Source::Auto => {
            // v1.0.0 behavior preserved: try UDP :5005 first, fall back to seed-stream.
            let udp = try_udp_csi();
            if !udp.is_empty() {
                (udp, "udp_csi")
            } else {
                let sensors = fetch_sensors()?;
                let samples = sensors.get("samples").and_then(|c| c.as_array()).ok_or("no samples")?;
                let vals: Vec<f64> = samples.iter()
                    .filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
                (vals, "sensor_api")
            }
        }
        Source::SeedStream => {
            let sensors = fetch_sensors()?;
            let samples = sensors.get("samples").and_then(|c| c.as_array()).ok_or("no samples")?;
            let vals: Vec<f64> = samples.iter()
                .filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
            (vals, "sensor_api")
        }
        Source::Esp32Uart(path) => (fetch_from_esp32_uart(path, window_ms, 256)?, "esp32-uart"),
        Source::Esp32Udp(addr) => (fetch_from_esp32_udp(addr, window_ms, 256)?, "esp32-udp"),
    };
    let source_str = src_tag;

    if features.is_empty() {
        return Err("no CSI data available".into());
    }

    // Accumulate into channel buffers
    while state.channel_buffers.len() < features.len() {
        state.channel_buffers.push(Vec::new());
    }
    for (i, &val) in features.iter().enumerate() {
        if i < state.channel_buffers.len() {
            state.channel_buffers[i].push(val);
            if state.channel_buffers[i].len() > state.buffer_max {
                state.channel_buffers[i].remove(0);
            }
        }
    }

    // Apply Hampel filter to each channel buffer
    let mut total_corrections = 0usize;
    for buf in &mut state.channel_buffers {
        let before = buf.clone();
        hampel_filter(buf);
        total_corrections += before.iter().zip(buf.iter())
            .filter(|(&a, &b)| (a - b).abs() > 1e-10)
            .count();
    }

    // Use latest filtered values
    let mut filtered: Vec<f64> = state.channel_buffers.iter()
        .filter_map(|buf| buf.last().copied())
        .collect();

    // Phase sanitization
    phase_sanitize(&mut filtered);

    let raw_count = filtered.len();

    // Map to keypoints
    let keypoints = csi_to_keypoints(&filtered);
    let avg_conf = if keypoints.is_empty() { 0.0 } else {
        keypoints.iter().map(|k| k.confidence).sum::<f64>() / keypoints.len() as f64
    };

    // Store as 8-dim feature vector (PCA-like compression)
    let mut vec8 = [0.0f64; 8];
    for (i, &v) in filtered.iter().take(8).enumerate() {
        vec8[i] = v;
    }
    // Normalize
    let norm: f64 = vec8.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 1e-10 {
        for v in &mut vec8 { *v /= norm; }
    }
    let _ = store_vector(&vec8);

    Ok(DensePoseReport {
        num_keypoints: keypoints.len(),
        avg_confidence: avg_conf,
        keypoints,
        source: source_str.to_string(),
        raw_features: raw_count,
        hampel_corrections: total_corrections,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    let source_spec = args.iter()
        .position(|a| a == "--source")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "auto".into());
    let source = match parse_source_arg(&source_spec) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[cog-ruview-densepose] {}", e);
            std::process::exit(2);
        }
    };
    let window_ms: u64 = args.iter()
        .position(|a| a == "--window-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);

    eprintln!("[cog-ruview-densepose] starting (interval={interval}s source={source_spec} window_ms={window_ms})");
    let mut state = DensePoseState::new();

    loop {
        let start = Instant::now();
        match run_once(&mut state, &source, window_ms) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-ruview-densepose] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
