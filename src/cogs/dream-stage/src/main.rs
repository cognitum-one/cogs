//! Cognitum Cog: Dream Stage (Sleep Stage Classifier)
//!
//! Classifies sleep stages from movement/breathing patterns using 60s
//! sliding windows:
//!   - Deep sleep: low variance + slow breathing (<0.2 Hz dominant)
//!   - REM: irregular breathing + micro-movements (moderate HF energy)
//!   - Light sleep: moderate variance
//!   - Wake: high variance + high movement
//!
//! Usage:
//!   cog-dream-stage --once
//!   cog-dream-stage --interval 60

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

/// Ring buffer for sliding window accumulation
struct SlidingWindow {
    buffer: Vec<f64>,
    capacity: usize,
    pos: usize,
    full: bool,
}

impl SlidingWindow {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            capacity,
            pos: 0,
            full: false,
        }
    }

    fn push(&mut self, value: f64) {
        self.buffer[self.pos] = value;
        self.pos += 1;
        if self.pos >= self.capacity {
            self.pos = 0;
            self.full = true;
        }
    }

    fn is_ready(&self) -> bool { self.full }

    fn data(&self) -> Vec<f64> {
        if self.full {
            let mut out = Vec::with_capacity(self.capacity);
            out.extend_from_slice(&self.buffer[self.pos..]);
            out.extend_from_slice(&self.buffer[..self.pos]);
            out
        } else {
            self.buffer[..self.pos].to_vec()
        }
    }
}

fn signal_variance(data: &[f64]) -> f64 {
    if data.len() < 2 { return 0.0; }
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (data.len() - 1) as f64
}

fn signal_energy(data: &[f64]) -> f64 {
    if data.is_empty() { return 0.0; }
    data.iter().map(|v| v * v).sum::<f64>() / data.len() as f64
}

/// Count zero crossings to estimate dominant frequency
fn zero_crossing_hz(signal: &[f64], sample_rate: f64) -> f64 {
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
    crossings as f64 / (2.0 * duration_s)
}

/// Breathing regularity: coefficient of variation of inter-breath intervals
fn breathing_regularity(signal: &[f64]) -> f64 {
    // Find positive zero-crossings as breath markers
    let mut crossings = Vec::new();
    for i in 1..signal.len() {
        if signal[i - 1] < 0.0 && signal[i] >= 0.0 {
            crossings.push(i);
        }
    }
    if crossings.len() < 3 { return 0.0; }
    let intervals: Vec<f64> = crossings.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    if mean < 1e-6 { return 0.0; }
    let var = intervals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
    // Return regularity: 1.0 = perfectly regular, 0.0 = very irregular
    1.0 - (var.sqrt() / mean).min(1.0)
}

#[derive(serde::Serialize)]
struct SleepReport {
    stage: String,
    stage_code: u8,
    movement_variance: f64,
    breathing_rate_hz: f64,
    breathing_regularity: f64,
    hf_energy_ratio: f64,
    confidence: f64,
    window_samples: usize,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
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
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_report(report: &SleepReport) -> Result<(), String> {
    let vector = vec![
        report.stage_code as f64 / 4.0,
        report.movement_variance.min(1.0),
        report.breathing_rate_hz / 0.5,
        report.breathing_regularity,
        report.hf_energy_ratio.min(1.0),
        report.confidence,
        0.0, 0.0,
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
        .unwrap_or(60);

    eprintln!("[cog-dream-stage] starting (interval={}s)", interval);

    let sample_rate = 10.0;
    // 60s window at 10Hz = 600 samples
    let window_size = 600;
    let mut window = SlidingWindow::new(window_size);
    let mut breathing_filter = BandpassFilter::new(0.1, 0.5, sample_rate);
    let mut hf_filter = BandpassFilter::new(1.0, 4.0, sample_rate);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    for ch in chs {
                        if let Some(val) = ch.get("value").and_then(|v| v.as_f64()) {
                            window.push(val);
                        }
                    }

                    if window.is_ready() || once {
                        let data = window.data();
                        if data.len() >= 10 {
                            // Feature extraction
                            let movement_var = signal_variance(&data);

                            let breathing: Vec<f64> = data.iter()
                                .map(|&v| breathing_filter.process(v))
                                .collect();
                            let hf_signal: Vec<f64> = data.iter()
                                .map(|&v| hf_filter.process(v))
                                .collect();

                            let br_hz = zero_crossing_hz(&breathing, sample_rate);
                            let br_regularity = breathing_regularity(&breathing);
                            let total_energy = signal_energy(&data).max(1e-15);
                            let hf_energy = signal_energy(&hf_signal);
                            let hf_ratio = hf_energy / total_energy;

                            // Classification logic
                            let (stage, code, confidence) = if movement_var > 0.5 {
                                // High movement = wake
                                ("wake", 0_u8, (movement_var / 1.0).min(1.0))
                            } else if movement_var < 0.02 && br_hz < 0.2 && br_regularity > 0.7 {
                                // Very low variance + slow regular breathing = deep
                                let c = br_regularity * (1.0 - movement_var * 10.0).max(0.0);
                                ("deep", 3, c.min(1.0))
                            } else if br_regularity < 0.5 && hf_ratio > 0.1 && movement_var < 0.15 {
                                // Irregular breathing + micro-movements = REM
                                let c = (1.0 - br_regularity) * hf_ratio.min(1.0);
                                ("rem", 2, c.min(1.0))
                            } else {
                                // Everything else = light sleep
                                let c = 0.5;
                                ("light", 1, c)
                            };

                            let report = SleepReport {
                                stage: stage.into(),
                                stage_code: code,
                                movement_variance: movement_var,
                                breathing_rate_hz: br_hz,
                                breathing_regularity: br_regularity,
                                hf_energy_ratio: hf_ratio,
                                confidence,
                                window_samples: data.len(),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            };

                            println!("{}", serde_json::to_string(&report).unwrap_or_default());
                            if let Err(e) = store_report(&report) {
                                eprintln!("[cog-dream-stage] store error: {e}");
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("[cog-dream-stage] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
