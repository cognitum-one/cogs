//! Cognitum Cog: Health Monitor (Combined Dashboard)
//!
//! Runs vital-trend + presence + apnea detection in a single process.
//! Outputs unified health JSON with all metrics per reading.
//!
//! Usage:
//!   cog-health-monitor [--once] [--interval 10] [--source SOURCE]
//!
//! Sources (--source):
//!   seed-stream            (default — read 127.0.0.1:80/api/v1/sensor/stream)
//!   esp32-uart=<path>      ESP32 serial port; <path> = "COM8" / "/dev/ttyACM0"
//!                          Build with --features esp32-uart.
//!   esp32-udp=<host:port>  Bind UDP, parse ADR-069 packets (0xC5110003).
//!                          Build with --features esp32-udp.
//!
//! ADR-091: cogs are self-contained. The cog brings its own sensor source
//! rather than depending on the agent's sensor/stream aggregator.

use std::io::Read;
use std::time::{Duration, Instant};

struct BandpassFilter {
    a1: f64, a2: f64, b0: f64, b2: f64,
    x1: f64, x2: f64, y1: f64, y2: f64,
}

impl BandpassFilter {
    fn new(freq_low: f64, freq_high: f64, sample_rate: f64) -> Self {
        let omega_low = 2.0 * std::f64::consts::PI * freq_low / sample_rate;
        let omega_high = 2.0 * std::f64::consts::PI * freq_high / sample_rate;
        let center = (omega_low + omega_high) / 2.0;
        let bandwidth = omega_high - omega_low;
        let r = 1.0 - bandwidth / 2.0;
        let r2 = r * r;
        Self {
            a1: -2.0 * r * center.cos(), a2: r2,
            b0: 1.0 - r2, b2: -(1.0 - r2),
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        }
    }
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = input;
        self.y2 = self.y1; self.y1 = output;
        output
    }
}

struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordStats {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }
    fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }
    fn std_dev(&self) -> f64 { self.variance().sqrt() }
    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { 0.0 } else { (value - self.mean) / sd }
    }
}

fn zero_crossing_bpm(signal: &[f64], sample_rate: f64) -> f64 {
    if signal.len() < 4 { return 0.0; }
    let mut crossings = 0;
    for i in 1..signal.len() {
        if (signal[i - 1] >= 0.0 && signal[i] < 0.0) ||
           (signal[i - 1] < 0.0 && signal[i] >= 0.0) {
            crossings += 1;
        }
    }
    let duration_s = signal.len() as f64 / sample_rate;
    if duration_s < 0.1 { return 0.0; }
    (crossings as f64 / (2.0 * duration_s)) * 60.0
}

/// Presence detection via signal variance threshold
struct PresenceDetector {
    baseline_var: f64,
    baseline_count: u64,
    threshold: f64,
    debounce_on: u32,
    debounce_off: u32,
    on_count: u32,
    off_count: u32,
    is_present: bool,
}

impl PresenceDetector {
    fn new() -> Self {
        Self {
            baseline_var: 0.0, baseline_count: 0,
            threshold: 10.0,
            debounce_on: 3, debounce_off: 5,
            on_count: 0, off_count: 0, is_present: false,
        }
    }
    fn update(&mut self, variance: f64) -> bool {
        if self.baseline_count < 20 {
            self.baseline_count += 1;
            let alpha = 1.0 / self.baseline_count as f64;
            self.baseline_var = self.baseline_var * (1.0 - alpha) + variance * alpha;
        }
        let raw = variance > self.threshold;
        if raw { self.on_count += 1; self.off_count = 0; }
        else { self.off_count += 1; self.on_count = 0; }
        if self.on_count >= self.debounce_on { self.is_present = true; }
        if self.off_count >= self.debounce_off { self.is_present = false; }
        if !self.is_present {
            self.baseline_var = self.baseline_var * 0.99 + variance * 0.01;
        }
        self.is_present
    }
}

/// Apnea detector: breathing amplitude drop
struct ApneaDetector {
    baseline_amp: f64,
    count: u64,
    event_frames: u64,
}

impl ApneaDetector {
    fn new() -> Self { Self { baseline_amp: 0.0, count: 0, event_frames: 0 } }
    fn update(&mut self, breathing_amp: f64) -> (bool, f64) {
        self.count += 1;
        if self.count <= 30 {
            let alpha = 1.0 / self.count as f64;
            self.baseline_amp = self.baseline_amp * (1.0 - alpha) + breathing_amp * alpha;
            return (false, 0.0);
        }
        let drop = if self.baseline_amp > 1e-6 {
            1.0 - breathing_amp / self.baseline_amp
        } else { 0.0 };
        if drop > 0.50 {
            self.event_frames += 1;
        } else {
            self.event_frames = 0;
            self.baseline_amp = self.baseline_amp * 0.99 + breathing_amp * 0.01;
        }
        // Apnea if sustained drop for 3+ frames
        (self.event_frames >= 3, drop)
    }
}

#[derive(serde::Serialize)]
struct HealthReport {
    // Vitals
    breathing_bpm: f64,
    heart_rate_bpm: f64,
    // Presence
    presence_detected: bool,
    signal_variance: f64,
    // Apnea
    apnea_detected: bool,
    breathing_drop_pct: f64,
    // Combined
    overall_status: String,
    alerts: Vec<String>,
    timestamp: u64,
}

// ── Sensor sources (ADR-091) ─────────────────────────────────────────

/// Where the cog reads samples from. Selected at startup via `--source`;
/// the same DSP pipeline consumes samples regardless of source.
enum Source {
    /// Default — read agent's `/api/v1/sensor/stream` over loopback HTTP.
    SeedStream,
    /// Direct ESP32 serial port (115200 8N1). Requires `--features esp32-uart`.
    /// Param: device path (`COM8` on Windows, `/dev/ttyACM0` on Linux).
    Esp32Uart(String),
    /// Bind UDP for ADR-069 feature packets. Requires `--features esp32-udp`.
    /// Param: `bind_host:port` (e.g. `0.0.0.0:5006`).
    Esp32Udp(String),
}

/// A normalized batch of samples for the DSP pipeline. Values are in
/// `[-1.0, 1.0]`. The DSP code only needs raw amplitudes — it doesn't
/// care which source produced them.
struct SensorBatch {
    /// Per-channel amplitude samples (normalized).
    amplitudes: Vec<f64>,
    /// Tag for the report's `sensor` field — useful when comparing
    /// `seed-stream` (synthetic) vs real ESP32 output side-by-side.
    source_tag: &'static str,
}

fn parse_source_arg(spec: &str) -> Result<Source, String> {
    if spec == "seed-stream" {
        return Ok(Source::SeedStream);
    }
    if let Some(path) = spec.strip_prefix("esp32-uart=") {
        if path.is_empty() {
            return Err("esp32-uart= requires a path (e.g. COM8 or /dev/ttyACM0)".into());
        }
        return Ok(Source::Esp32Uart(path.to_string()));
    }
    if let Some(addr) = spec.strip_prefix("esp32-udp=") {
        if addr.is_empty() {
            return Err("esp32-udp= requires bind_host:port (e.g. 0.0.0.0:5006)".into());
        }
        return Ok(Source::Esp32Udp(addr.to_string()));
    }
    Err(format!(
        "unknown source '{}'; expected one of: seed-stream | esp32-uart=PATH | esp32-udp=HOST:PORT",
        spec
    ))
}

/// Fetch one sample batch from the configured source. Returns at most
/// `max_samples` amplitude readings — typically the most recent window.
fn fetch_batch(source: &Source, window_ms: u64, max_samples: usize) -> Result<SensorBatch, String> {
    match source {
        Source::SeedStream => fetch_from_seed_stream(max_samples),
        Source::Esp32Uart(path) => fetch_from_esp32_uart(path, window_ms, max_samples),
        Source::Esp32Udp(addr) => fetch_from_esp32_udp(addr, window_ms, max_samples),
    }
}

// ── Source: agent's sensor/stream (default, backward compatible) ────

fn fetch_from_seed_stream(max_samples: usize) -> Result<SensorBatch, String> {
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match conn.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => { buf.extend_from_slice(&tmp[..n]); if buf.len() > 262144 { break; } }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    let v: serde_json::Value = serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))?;
    let chs = v.get("samples").and_then(|c| c.as_array())
        .ok_or("no samples array in /api/v1/sensor/stream")?;
    let amps: Vec<f64> = chs.iter().take(max_samples)
        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
        .collect();
    if amps.is_empty() {
        return Err("sensor/stream returned 0 samples".into());
    }
    Ok(SensorBatch { amplitudes: amps, source_tag: "seed-stream" })
}

// ── Source: ESP32 serial-port direct read ───────────────────────────

#[cfg(feature = "esp32-uart")]
fn fetch_from_esp32_uart(path: &str, window_ms: u64, max_samples: usize) -> Result<SensorBatch, String> {
    use std::time::Duration;

    // Open serial at 115200 8N1 with a short read timeout; we'll loop until
    // window_ms elapses or we have enough samples.
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
    let deadline = std::time::Instant::now() + Duration::from_millis(window_ms);
    while std::time::Instant::now() < deadline && buf.len() < 65536 {
        match port.read(&mut tmp) {
            Ok(0) => continue,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(format!("uart read: {}", e)),
        }
    }

    // The esp32-csi-node firmware emits a line per CSI callback:
    //   I (12345) csi_collector: CSI cb #800: len=128 rssi=-51 ch=5
    // Extract every rssi=<int> token across the whole buffer in one pass.
    let text = String::from_utf8_lossy(&buf);
    let amps: Vec<f64> = text.split_whitespace()
        .filter_map(|t| t.strip_prefix("rssi="))
        .filter_map(|s| s.trim_end_matches(',').parse::<f64>().ok())
        // Normalize dBm to [-1, 1]: -100 dBm -> -1, -30 dBm -> +1
        .map(|dbm| ((dbm + 65.0) / 35.0).clamp(-1.0, 1.0))
        .take(max_samples)
        .collect();

    if amps.is_empty() {
        return Err(format!(
            "no rssi=N tokens found in {}B from {} — is the ESP32 esp32-csi-node firmware running?",
            buf.len(), path
        ));
    }
    Ok(SensorBatch { amplitudes: amps, source_tag: "esp32-uart" })
}

#[cfg(not(feature = "esp32-uart"))]
fn fetch_from_esp32_uart(_path: &str, _window_ms: u64, _max_samples: usize) -> Result<SensorBatch, String> {
    Err("--source esp32-uart not enabled in this build (rebuild with --features esp32-uart)".into())
}

// ── Source: ESP32 UDP (ADR-069 feature packets) ─────────────────────

#[cfg(feature = "esp32-udp")]
fn fetch_from_esp32_udp(addr: &str, window_ms: u64, max_samples: usize) -> Result<SensorBatch, String> {
    use std::net::UdpSocket;
    use std::time::Duration;

    const MAGIC_FEATURES: u32 = 0xC511_0003;
    const FEATURE_PKT_SIZE: usize = 48; // 4 + 1 + 1 + 2 + 8 + 8*4

    let socket = UdpSocket::bind(addr).map_err(|e| format!("bind {}: {}", addr, e))?;
    socket.set_read_timeout(Some(Duration::from_millis(window_ms.min(2000))))
        .map_err(|e| format!("set timeout: {}", e))?;

    let mut amps: Vec<f64> = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(window_ms);
    let mut pkt = [0u8; 256];
    while std::time::Instant::now() < deadline && amps.len() < max_samples {
        match socket.recv_from(&mut pkt) {
            Ok((n, _)) if n >= FEATURE_PKT_SIZE => {
                let magic = u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]);
                if magic != MAGIC_FEATURES { continue; }
                // Skip header (4+1+1+2+8 = 16 bytes), read 8 LE f32 features.
                for i in 0..8 {
                    let off = 16 + i * 4;
                    if off + 4 > n { break; }
                    let f = f32::from_le_bytes([pkt[off], pkt[off+1], pkt[off+2], pkt[off+3]]);
                    if f.is_finite() {
                        amps.push((f as f64).clamp(-1.0, 1.0));
                    }
                }
            }
            Ok(_) => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("udp recv: {}", e)),
        }
    }
    if amps.is_empty() {
        return Err(format!(
            "no ADR-069 feature packets received on {} within {}ms — is the ESP32 sending here?",
            addr, window_ms
        ));
    }
    Ok(SensorBatch { amplitudes: amps, source_tag: "esp32-udp" })
}

#[cfg(not(feature = "esp32-udp"))]
fn fetch_from_esp32_udp(_addr: &str, _window_ms: u64, _max_samples: usize) -> Result<SensorBatch, String> {
    Err("--source esp32-udp not enabled in this build (rebuild with --features esp32-udp)".into())
}

fn store_report(report: &HealthReport) -> Result<(), String> {
    let status_code = match report.overall_status.as_str() {
        "normal" => 0.0, "alert" => 0.7, "critical" => 1.0, _ => 0.3,
    };
    let vector = vec![
        report.breathing_bpm / 40.0,
        report.heart_rate_bpm / 200.0,
        if report.presence_detected { 1.0 } else { 0.0 },
        report.signal_variance.min(100.0) / 100.0,
        if report.apnea_detected { 1.0 } else { 0.0 },
        report.breathing_drop_pct,
        status_code,
        report.alerts.len() as f64 / 5.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);
    // --source SPEC; default = seed-stream (backward compat with v1.0.0)
    let source_spec = args.iter()
        .position(|a| a == "--source")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "seed-stream".into());
    let source = match parse_source_arg(&source_spec) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[cog-health-monitor] {}", e);
            std::process::exit(2);
        }
    };
    // Window for non-stream sources (UART read interval, UDP listen interval).
    // Default 1000ms gives a 100-sample window at the ESP32-csi-node default rate.
    let window_ms: u64 = args.iter()
        .position(|a| a == "--window-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);

    eprintln!("[cog-health-monitor] starting (interval={}s source={} window_ms={})",
              interval, source_spec, window_ms);

    let sample_rate = 10.0;
    let mut breathing_filter = BandpassFilter::new(0.1, 0.5, sample_rate);
    let mut hr_filter = BandpassFilter::new(0.8, 2.0, sample_rate);
    let mut presence = PresenceDetector::new();
    let mut apnea = ApneaDetector::new();
    let mut vital_stats = WelfordStats::new();

    loop {
        let start = Instant::now();
        match fetch_batch(&source, window_ms, 256) {
            Ok(batch) => {
                let amps = batch.amplitudes;
                let _source_tag = batch.source_tag; // available for future report enrichment
                {
                    if amps.is_empty() {
                        eprintln!("[cog-health-monitor] no sensor readings");
                    } else {
                        // Signal variance for presence
                        let mut var_stats = WelfordStats::new();
                        for &v in &amps { var_stats.update(v); }
                        let sig_var = var_stats.variance();
                        let is_present = presence.update(sig_var);

                        // Breathing extraction
                        let breathing: Vec<f64> = amps.iter().map(|&v| breathing_filter.process(v)).collect();
                        let breathing_bpm = zero_crossing_bpm(&breathing, sample_rate);
                        let breathing_amp = breathing.iter().map(|v| v.abs()).sum::<f64>() / breathing.len().max(1) as f64;

                        // Heart rate extraction
                        let hr_signal: Vec<f64> = amps.iter().map(|&v| hr_filter.process(v)).collect();
                        let heart_rate_bpm = zero_crossing_bpm(&hr_signal, sample_rate);

                        // Apnea check
                        let (apnea_detected, drop_pct) = apnea.update(breathing_amp);

                        // Track vital trend
                        vital_stats.update(breathing_bpm);

                        let mut alerts = Vec::new();
                        if apnea_detected {
                            alerts.push(format!("APNEA: breathing drop={:.0}%", drop_pct * 100.0));
                        }
                        if breathing_bpm > 30.0 {
                            alerts.push(format!("TACHYPNEA: {:.0} bpm", breathing_bpm));
                        }
                        if heart_rate_bpm > 100.0 {
                            alerts.push(format!("TACHYCARDIA: {:.0} bpm", heart_rate_bpm));
                        }
                        if heart_rate_bpm > 0.0 && heart_rate_bpm < 50.0 {
                            alerts.push(format!("BRADYCARDIA: {:.0} bpm", heart_rate_bpm));
                        }
                        if vital_stats.count > 5 {
                            let z = vital_stats.z_score(breathing_bpm);
                            if z.abs() > 2.5 {
                                alerts.push(format!("VITAL_ANOMALY: z={:.2}", z));
                            }
                        }

                        let overall = if alerts.iter().any(|a| a.starts_with("APNEA")) {
                            "critical"
                        } else if alerts.is_empty() {
                            "normal"
                        } else {
                            "alert"
                        };

                        let report = HealthReport {
                            breathing_bpm,
                            heart_rate_bpm,
                            presence_detected: is_present,
                            signal_variance: sig_var,
                            apnea_detected,
                            breathing_drop_pct: drop_pct,
                            overall_status: overall.into(),
                            alerts: alerts.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        };

                        println!("{}", serde_json::to_string(&report).unwrap_or_default());
                        if let Err(e) = store_report(&report) {
                            eprintln!("[cog-health-monitor] store error: {e}");
                        }
                        if !alerts.is_empty() {
                            eprintln!("[cog-health-monitor] ALERT: {:?}", alerts);
                        }
                    }
                }
            }
            Err(e) => eprintln!("[cog-health-monitor] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
