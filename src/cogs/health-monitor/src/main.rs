//! Cognitum Cog: Health Monitor (Combined Dashboard)
//!
//! Runs vital-trend + presence + apnea detection in a single process.
//! Outputs unified health JSON with all metrics per reading.
//!
//! Usage:
//!   cog-health-monitor [--once] [--interval 10] [--source SOURCE]
//!
//! Sources (--source):
//!   auto                   (default — ~800ms probe on UDP :5006 (ADR-069);
//!                          if no packets, fall back to seed-stream)
//!   seed-stream            agent's /api/v1/sensor/stream over loopback
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

/// Fixed-size sliding window returning the median of the last `cap` values.
/// WiFi-CSI vitals are estimated per-window and are spiky frame-to-frame; a
/// short median rejects single-cycle outliers without the lag of a long moving
/// average. Median (not mean) so one wild spike cannot drag the published rate.
struct SmoothWindow {
    cap: usize,
    buf: std::collections::VecDeque<f64>,
}

impl SmoothWindow {
    fn new(cap: usize) -> Self {
        Self { cap: cap.max(1), buf: std::collections::VecDeque::new() }
    }
    fn clear(&mut self) {
        self.buf.clear();
    }
    /// Push a sample and return the median over the (capped) window.
    fn push_median(&mut self, v: f64) -> f64 {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
        let mut s: Vec<f64> = self.buf.iter().copied().collect();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = s.len();
        if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 }
    }
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
    /// Data provenance for this reading:
    /// `auto:esp32-vitals` (device-computed vitals packet — preferred),
    /// `auto:esp32-udp` (DSP over real ESP32 CSI features),
    /// `auto:seed-stream` (synthetic fallback), or an explicit `--source`.
    /// Lets operators and support tell at a glance whether vitals came from the
    /// real contactless feed or the demo stream.
    source: String,
}

// ── Sensor sources (ADR-091) ─────────────────────────────────────────

/// Where the cog reads samples from. Selected at startup via `--source`;
/// the same DSP pipeline consumes samples regardless of source.
enum Source {
    /// Default — try UDP :5006 first (ADR-069 standard), fall back to
    /// seed-stream if nothing arrives within a short probe window. Lets
    /// a cog dropped on a fleet seed Just Work when an ESP32 on the same
    /// WiFi is unicasting CSI packets, without requiring per-deploy
    /// flag tweaks. Built-in even when neither feature flag is enabled
    /// (the UDP path is gated behind cfg, but auto falls back gracefully).
    Auto,
    /// Read agent's `/api/v1/sensor/stream` over loopback HTTP.
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
    if spec == "auto" || spec.is_empty() {
        return Ok(Source::Auto);
    }
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
        "unknown source '{}'; expected one of: auto | seed-stream | esp32-uart=PATH | esp32-udp=HOST:PORT",
        spec
    ))
}

/// Default UDP bind for `auto` mode — ADR-069 standard port.
const AUTO_UDP_BIND: &str = "0.0.0.0:5006";
/// Probe window for `auto`: time spent waiting for UDP before falling
/// back to seed-stream. 2 s comfortably catches 1 Hz senders (the
/// scripts/esp32-uart-to-udp-bridge.py default cadence) while still
/// fitting inside a typical --interval 10 cycle.
const AUTO_UDP_PROBE_MS: u64 = 2000;

/// Fetch one sample batch from the configured source. Returns at most
/// `max_samples` amplitude readings — typically the most recent window.
fn fetch_batch(source: &Source, window_ms: u64, max_samples: usize) -> Result<SensorBatch, String> {
    match source {
        Source::Auto => {
            // ADR-091 default: prefer real ESP32 CSI over the agent's
            // (often-synthetic) sensor stream. Try a quick UDP probe;
            // if no packets arrive, fall back to seed-stream so cogs on
            // synthetic-only seeds still produce output instead of
            // hanging on an empty UDP socket.
            //
            // Use the SHARED, always-compiled probe from cog-sensor-sources.
            // The local fetch_from_esp32_udp is #[cfg(feature = "esp32-udp")]
            // and ships stubbed to an Err in the default registry build
            // (Dockerfile.cog-batch: `cargo build --release`, no --features),
            // so Auto used to fall straight through to synthetic and never
            // bound 5006. The shared crate has no such gate — same path the
            // cardiac-arrhythmia / sleep-apnea / respiratory-distress cogs use.
            match cog_sensor_sources::fetch_from_udp_window(&cog_sensor_sources::csi_bind_addr(), AUTO_UDP_PROBE_MS) {
                Ok(vals) if !vals.is_empty() => Ok(SensorBatch {
                    amplitudes: vals.into_iter().take(max_samples).collect(),
                    source_tag: "auto:esp32-udp",
                }),
                _ => {
                    let mut b = fetch_from_seed_stream(max_samples)?;
                    b.source_tag = "auto:seed-stream";
                    Ok(b)
                }
            }
        }
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

/// Field-model presence published by the `presence-field` broker cog (ADR-151).
/// Returns `Some(present)` only when the file exists and is fresh (< 8 s old), so
/// HM gates on robust multi-node presence instead of the device's flaky flag, and
/// falls back to its own presence when the broker isn't running.
fn read_field_presence() -> Option<bool> {
    let path = std::env::var("COG_PRESENCE_FILE")
        .unwrap_or_else(|_| "/tmp/cognitum-presence.json".to_string());
    let data = std::fs::read(&path).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&data).ok()?;
    let ts = v.get("ts")?.as_u64()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    if now.saturating_sub(ts) > 8 {
        return None; // stale — broker stopped; fall back to HM's own presence
    }
    v.get("present")?.as_bool()
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
    // --source SPEC; default = auto (ADR-091: prefer ESP32 UDP, fall
    // back to seed-stream if no UDP packets arrive within the probe
    // window). Existing v1.0.0 callers using only `--once`/`--interval`
    // (no `--source`) get the smarter behavior automatically; the
    // `seed-stream` source is still selectable by name for full
    // backward compat.
    let source_spec = args.iter()
        .position(|a| a == "--source")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "auto".into());
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

    // Interleave fix: for `auto`, hold ONE persistently-bound UDP listener and
    // drain it each cycle. The socket stays open, so the kernel buffers ESP32
    // packets between cycles instead of dropping them (the gaps that used to
    // force a synthetic fall-back ~half the time and corrupt the stateful DSP).
    // Once the feed is live we NEVER substitute synthetic — we skip the cycle on
    // a brief gap, and only resume seed-stream after the feed is absent for
    // ESP32_GAP_TOLERANCE consecutive cycles (i.e. the ESP32 looks offline).
    const ESP32_GAP_TOLERANCE: u32 = 3;
    // Hysteresis over the device presence flag (it flickers at close range —
    // RuView#996): latch present immediately, clear only after N absent frames.
    const PRESENCE_CLEAR_AFTER: u32 = 3;
    // Apnea is only declared after breathing stays sub-threshold for this many
    // consecutive present cycles — avoids a false alarm on the transient 0 a
    // person shows the instant they're detected (before the first estimate).
    // NOTE: whether the device reports breathing==0 for true cessation vs
    // "no estimate yet" needs firmware confirmation; the sustained gate is
    // conservative either way.
    const APNEA_CONFIRM_CYCLES: u32 = 3;
    // High-rate alarms (TACHYPNEA/TACHYCARDIA) require the elevation to persist
    // for this many consecutive present cycles before firing — mirrors the
    // APNEA gate so a single noisy per-window estimate can't raise a false
    // clinical alert (cogs#41).
    const TACHYPNEA_CONFIRM_CYCLES: u32 = 3;
    const TACHYCARDIA_CONFIRM_CYCLES: u32 = 3;
    // Median window over the published breathing/HR rate. WiFi-CSI vitals are
    // estimated per-window and swing cycle-to-cycle; a short median rejects
    // single-cycle outliers before display/alerting (cogs#41).
    const VITALS_SMOOTH_WINDOW: usize = 5;
    let udp_listener = match &source {
        Source::Auto => match cog_sensor_sources::Esp32UdpListener::bind(&cog_sensor_sources::csi_bind_addr()) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!(
                    "[cog-health-monitor] WARNING: cannot bind {AUTO_UDP_BIND} ({e}); \
                     another CSI cog likely holds it — serving synthetic until it frees"
                );
                None
            }
        },
        _ => None,
    };
    let mut esp32_seen = false;
    let mut gap_streak: u32 = 0;
    let mut presence_sticky = false;
    let mut presence_absent_streak: u32 = 0;
    let mut low_breath_streak: u32 = 0;
    // cogs#41: sustained high-rate alarm gates + per-rate median smoothers.
    let mut high_breath_streak: u32 = 0;
    let mut high_hr_streak: u32 = 0;
    let mut breath_smooth = SmoothWindow::new(VITALS_SMOOTH_WINDOW);
    let mut hr_smooth = SmoothWindow::new(VITALS_SMOOTH_WINDOW);

    // One acquired cycle is either device-computed vitals (preferred — the ESP32
    // already runs the estimation, v0.7.1-validated) or raw amplitude samples to
    // run the cog's own DSP over (feature feed or synthetic seed-stream).
    enum Acquired {
        Vitals(cog_sensor_sources::Esp32Vitals),
        Samples { amps: Vec<f64>, source_tag: &'static str },
    }
    // Breathing below this (BPM) is treated as apnea on the device-vitals path.
    const APNEA_BREATH_MIN_BPM: f64 = 5.0;

    loop {
        let start = Instant::now();
        // Acquire one cycle. `Ok(None)` means "skip" (an ESP32 gap we
        // deliberately do not paper over with synthetic data).
        let acquired: Result<Option<Acquired>, String> = match udp_listener.as_ref() {
            Some(listener) => {
                let frame = listener.drain(256);
                if let Some(v) = frame.vitals {
                    // Preferred: device already computed presence + vitals.
                    esp32_seen = true;
                    gap_streak = 0;
                    Ok(Some(Acquired::Vitals(v)))
                } else if !frame.features.is_empty() {
                    esp32_seen = true;
                    gap_streak = 0;
                    Ok(Some(Acquired::Samples { amps: frame.features, source_tag: "auto:esp32-udp" }))
                } else if esp32_seen && gap_streak < ESP32_GAP_TOLERANCE {
                    gap_streak += 1;
                    Ok(None) // brief gap in a live feed — skip, keep DSP clean
                } else {
                    // No ESP32 yet (synthetic-only seed) or feed gone for a while:
                    // fall back to seed-stream so the cog still produces output.
                    esp32_seen = false;
                    gap_streak = gap_streak.saturating_add(1);
                    fetch_from_seed_stream(256).map(|b| {
                        Some(Acquired::Samples { amps: b.amplitudes, source_tag: "auto:seed-stream" })
                    })
                }
            }
            None => fetch_batch(&source, window_ms, 256)
                .map(|b| Some(Acquired::Samples { amps: b.amplitudes, source_tag: b.source_tag })),
        };
        match acquired {
            Ok(Some(acq)) => {
                // Reduce either source to a common reading, then run the shared
                // alert + report path. `None` = nothing to emit this cycle.
                struct Reading {
                    breathing_bpm: f64,
                    heart_rate_bpm: f64,
                    is_present: bool,
                    sig_var: f64,
                    apnea_detected: bool,
                    drop_pct: f64,
                    source_tag: &'static str,
                }
                let reading: Option<Reading> = match acq {
                    Acquired::Vitals(v) => {
                        // Trust the device's on-board estimate (v0.7.1-validated):
                        // presence, heart rate and breathing come straight from
                        // the ESP32 rather than being re-derived from raw features.
                        vital_stats.update(v.breathing_bpm);

                        // Debounce the flickery device presence flag (RuView#996):
                        // latch on immediately, clear only after N absent frames.
                        if v.presence {
                            presence_sticky = true;
                            presence_absent_streak = 0;
                        } else {
                            presence_absent_streak += 1;
                            if presence_absent_streak >= PRESENCE_CLEAR_AFTER {
                                presence_sticky = false;
                            }
                        }

                        // Apnea = a PRESENT person whose breathing has dropped to
                        // ~zero. breathing==0 is the alarm (must be included), but
                        // require it sustained for APNEA_CONFIRM_CYCLES so the
                        // transient 0 at first detection doesn't false-alarm.
                        if presence_sticky && v.breathing_bpm < APNEA_BREATH_MIN_BPM {
                            low_breath_streak += 1;
                        } else {
                            low_breath_streak = 0;
                        }
                        let apnea_detected = low_breath_streak >= APNEA_CONFIRM_CYCLES;

                        Some(Reading {
                            breathing_bpm: v.breathing_bpm,
                            heart_rate_bpm: v.heart_rate_bpm,
                            is_present: presence_sticky,
                            sig_var: v.presence_score as f64,
                            apnea_detected,
                            drop_pct: 0.0,
                            source_tag: "auto:esp32-vitals",
                        })
                    }
                    Acquired::Samples { amps, source_tag } => {
                        if amps.is_empty() {
                            eprintln!("[cog-health-monitor] no sensor readings");
                            None
                        } else {
                            // Signal variance for presence
                            let mut var_stats = WelfordStats::new();
                            for &v in &amps { var_stats.update(v); }
                            let sig_var = var_stats.variance();
                            let is_present = presence.update(sig_var);

                            // Breathing extraction
                            let breathing: Vec<f64> =
                                amps.iter().map(|&v| breathing_filter.process(v)).collect();
                            let breathing_bpm = zero_crossing_bpm(&breathing, sample_rate);
                            let breathing_amp = breathing.iter().map(|v| v.abs()).sum::<f64>()
                                / breathing.len().max(1) as f64;

                            // Heart rate extraction
                            let hr_signal: Vec<f64> =
                                amps.iter().map(|&v| hr_filter.process(v)).collect();
                            let heart_rate_bpm = zero_crossing_bpm(&hr_signal, sample_rate);

                            // Apnea check
                            let (apnea_detected, drop_pct) = apnea.update(breathing_amp);

                            // Track vital trend
                            vital_stats.update(breathing_bpm);

                            Some(Reading {
                                breathing_bpm,
                                heart_rate_bpm,
                                is_present,
                                sig_var,
                                apnea_detected,
                                drop_pct,
                                source_tag,
                            })
                        }
                    }
                };

                if let Some(mut r) = reading {
                    // ADR-151: prefer the field-model broker's robust multi-node
                    // presence over the device's unreliable single-node flag, when
                    // a fresh reading is available. Gates vitals/alerts on it.
                    if let Some(fp) = read_field_presence() {
                        r.is_present = fp;
                        if !fp { r.apnea_detected = false; } // no person -> no apnea alarm
                    }

                    // cogs#41: temporal median smoothing of the published rates.
                    // Per-window CSI vitals are spiky; publish a short-window
                    // median so a single noisy estimate can't spike the displayed
                    // rate or trip an alarm. Only smooth a present reading; reset
                    // on absence so stale values don't carry across an empty room.
                    if r.is_present {
                        r.breathing_bpm = breath_smooth.push_median(r.breathing_bpm);
                        r.heart_rate_bpm = hr_smooth.push_median(r.heart_rate_bpm);
                    } else {
                        breath_smooth.clear();
                        hr_smooth.clear();
                    }

                    let mut alerts = Vec::new();
                    if r.apnea_detected {
                        alerts.push(format!("APNEA: breathing drop={:.0}%", r.drop_pct * 100.0));
                    }
                    // cogs#41: high-rate alarms require SUSTAINED elevation (N
                    // consecutive present cycles), mirroring the apnea gate — a
                    // transient artifact spike no longer raises a false alert. No
                    // vitals alarms while no person is present.
                    if r.is_present {
                        if r.breathing_bpm > 30.0 { high_breath_streak += 1; } else { high_breath_streak = 0; }
                        if high_breath_streak >= TACHYPNEA_CONFIRM_CYCLES {
                            alerts.push(format!("TACHYPNEA: {:.0} bpm", r.breathing_bpm));
                        }
                        if r.heart_rate_bpm > 100.0 { high_hr_streak += 1; } else { high_hr_streak = 0; }
                        if high_hr_streak >= TACHYCARDIA_CONFIRM_CYCLES {
                            alerts.push(format!("TACHYCARDIA: {:.0} bpm", r.heart_rate_bpm));
                        }
                        if r.heart_rate_bpm > 0.0 && r.heart_rate_bpm < 50.0 {
                            alerts.push(format!("BRADYCARDIA: {:.0} bpm", r.heart_rate_bpm));
                        }
                        if vital_stats.count > 5 {
                            let z = vital_stats.z_score(r.breathing_bpm);
                            if z.abs() > 2.5 {
                                alerts.push(format!("VITAL_ANOMALY: z={:.2}", z));
                            }
                        }
                    } else {
                        high_breath_streak = 0;
                        high_hr_streak = 0;
                    }

                    let overall = if alerts.iter().any(|a| a.starts_with("APNEA")) {
                        "critical"
                    } else if alerts.is_empty() {
                        "normal"
                    } else {
                        "alert"
                    };

                    let report = HealthReport {
                        breathing_bpm: r.breathing_bpm,
                        heart_rate_bpm: r.heart_rate_bpm,
                        presence_detected: r.is_present,
                        signal_variance: r.sig_var,
                        apnea_detected: r.apnea_detected,
                        breathing_drop_pct: r.drop_pct,
                        overall_status: overall.into(),
                        alerts: alerts.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        source: r.source_tag.to_string(),
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
            Ok(None) => { /* ESP32 gap — skip this cycle; never feed synthetic into the DSP */ }
            Err(e) => eprintln!("[cog-health-monitor] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SmoothWindow;

    #[test]
    fn median_rejects_a_single_spike() {
        // Resting breathing ~16 bpm with one artifact spike to 33.
        let mut w = SmoothWindow::new(5);
        for v in [16.0, 17.0, 16.0, 33.0, 15.0] {
            w.push_median(v);
        }
        // Median over the window ignores the lone 33 spike.
        let m = w.push_median(16.0);
        assert!(m < 20.0, "median {m} should stay near resting rate, not chase the spike");
    }

    #[test]
    fn median_tracks_a_sustained_shift() {
        // A genuine sustained rise must come through (not be suppressed).
        let mut w = SmoothWindow::new(5);
        let mut last = 0.0;
        for v in [12.0, 13.0, 28.0, 29.0, 30.0, 31.0, 32.0] {
            last = w.push_median(v);
        }
        assert!(last >= 29.0, "sustained elevation {last} should propagate through the median");
    }

    #[test]
    fn clear_resets_the_window() {
        let mut w = SmoothWindow::new(3);
        w.push_median(40.0);
        w.push_median(41.0);
        w.clear();
        // After clear, the first new sample is its own median.
        assert_eq!(w.push_median(15.0), 15.0);
    }

    #[test]
    fn window_never_exceeds_cap() {
        let mut w = SmoothWindow::new(3);
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            w.push_median(v);
        }
        // Last 3 are [3,4,5] -> median 4.
        assert_eq!(w.push_median(6.0), 5.0); // window [4,5,6] -> 5
    }
}
