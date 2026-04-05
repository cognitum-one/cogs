//! Cognitum Cog: Rain Detection
//!
//! Detects rain from signal scattering patterns. Rain causes broadband
//! amplitude increase with characteristic noise floor rise.
//! Classifies: none / light / moderate / heavy.
//!
//! Usage:
//!   cog-rain-detect --once
//!   cog-rain-detect --interval 30

use std::io::Read;
use std::time::{Duration, Instant};

const NOISE_HISTORY: usize = 60;

struct NoiseFloorTracker {
    /// Rolling noise floor estimates per channel
    floors: Vec<f64>,
    /// History of broadband amplitude
    amplitude_history: Vec<f64>,
}

impl NoiseFloorTracker {
    fn new() -> Self {
        Self { floors: Vec::new(), amplitude_history: Vec::new() }
    }

    fn update(&mut self, values: &[f64]) {
        // Noise floor: exponential moving minimum
        if self.floors.len() != values.len() {
            self.floors = values.to_vec();
        }
        for (i, &v) in values.iter().enumerate() {
            // Slowly adapt floor upward, quickly track downward
            if v < self.floors[i] {
                self.floors[i] = v;
            } else {
                self.floors[i] = self.floors[i] * 0.99 + v * 0.01;
            }
        }

        // Track broadband amplitude (mean across channels)
        let mean_amp = if values.is_empty() { 0.0 } else {
            values.iter().sum::<f64>() / values.len() as f64
        };
        self.amplitude_history.push(mean_amp);
        if self.amplitude_history.len() > NOISE_HISTORY {
            self.amplitude_history.remove(0);
        }
    }

    /// Noise floor rise ratio: current vs baseline
    fn noise_rise_ratio(&self, values: &[f64]) -> f64 {
        if self.floors.is_empty() || values.len() != self.floors.len() { return 0.0; }
        let mut ratio_sum = 0.0;
        let mut count = 0;
        for (i, &v) in values.iter().enumerate() {
            if self.floors[i].abs() > 1e-6 {
                ratio_sum += v / self.floors[i];
                count += 1;
            }
        }
        if count == 0 { 1.0 } else { ratio_sum / count as f64 }
    }

    /// Broadband uniformity: how evenly the amplitude increase is distributed
    fn broadband_uniformity(values: &[f64]) -> f64 {
        if values.len() < 2 { return 0.0; }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        if mean < 1e-10 { return 0.0; }
        let cv = (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt() / mean;
        // Low CV = uniform (rain-like), High CV = not uniform
        (1.0 - cv).max(0.0)
    }

    /// Amplitude variability (rain has characteristic high-freq noise)
    fn amplitude_variability(&self) -> f64 {
        if self.amplitude_history.len() < 3 { return 0.0; }
        let diffs: Vec<f64> = self.amplitude_history.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .collect();
        let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
        mean
    }
}

#[derive(serde::Serialize)]
struct RainReport {
    classification: String,
    rain_score: f64,
    noise_rise_ratio: f64,
    broadband_uniformity: f64,
    amplitude_variability: f64,
    mean_amplitude: f64,
    noise_floor: f64,
    timestamp: u64,
}

fn classify_rain(score: f64) -> &'static str {
    if score < 0.2 { "none" }
    else if score < 0.4 { "light" }
    else if score < 0.7 { "moderate" }
    else { "heavy" }
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
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON in response")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(vec8: [f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec8]], "dedup": true });
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

fn run_once(tracker: &mut NoiseFloorTracker) -> Result<RainReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples in sensor data")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    if values.is_empty() {
        return Err("no sensor readings".into());
    }

    tracker.update(&values);

    let noise_rise = tracker.noise_rise_ratio(&values);
    let uniformity = NoiseFloorTracker::broadband_uniformity(&values);
    let variability = tracker.amplitude_variability();

    // Rain score: weighted combination
    // High noise rise + high uniformity + moderate variability = rain
    let rise_component = ((noise_rise - 1.0) / 2.0).max(0.0).min(1.0); // 0-1 scale
    let var_component = (variability / 10.0).min(1.0);
    let rain_score = (rise_component * 0.4 + uniformity * 0.3 + var_component * 0.3).min(1.0);

    let mean_amp = values.iter().sum::<f64>() / values.len() as f64;
    let noise_floor = tracker.floors.iter().sum::<f64>() / tracker.floors.len().max(1) as f64;

    let report = RainReport {
        classification: classify_rain(rain_score).into(),
        rain_score,
        noise_rise_ratio: noise_rise,
        broadband_uniformity: uniformity,
        amplitude_variability: variability,
        mean_amplitude: mean_amp,
        noise_floor,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        rain_score,
        noise_rise / 5.0,
        uniformity,
        variability / 20.0,
        mean_amp / 100.0,
        noise_floor / 100.0,
        0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-rain-detect] store error: {e}");
    }

    Ok(report)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    eprintln!("[cog-rain-detect] starting (interval={}s)", interval);

    let mut tracker = NoiseFloorTracker::new();

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.rain_score > 0.2 {
                    eprintln!("[cog-rain-detect] ALERT: {} rain detected (score={:.2})",
                        report.classification, report.rain_score);
                }
            }
            Err(e) => eprintln!("[cog-rain-detect] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
