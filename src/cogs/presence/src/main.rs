//! Cognitum Cog: Presence Detection
//!
//! Detects human presence from WiFi CSI signal variance using z-score
//! on RSSI with temporal debounce to avoid flicker.
//!
//! Usage:
//!   cog-presence --once           # Single check
//!   cog-presence                  # Continuous (5s)
//!   cog-presence --interval 3    # Custom interval

use std::io::Read;
use std::time::{Duration, Instant};

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
}

/// Debounced presence state machine
struct PresenceDetector {
    baseline_stats: WelfordStats,
    variance_threshold: f64,
    debounce_on: u32,   // consecutive frames needed to confirm present
    debounce_off: u32,  // consecutive frames needed to confirm absent
    on_count: u32,
    off_count: u32,
    is_present: bool,
    total_detections: u64,
}

impl PresenceDetector {
    fn new(variance_threshold: f64) -> Self {
        Self {
            baseline_stats: WelfordStats::new(),
            variance_threshold,
            debounce_on: 3,
            debounce_off: 5,
            on_count: 0,
            off_count: 0,
            is_present: false,
            total_detections: 0,
        }
    }

    fn update(&mut self, signal_variance: f64) -> bool {
        let raw_present = signal_variance > self.variance_threshold;

        if raw_present {
            self.on_count += 1;
            self.off_count = 0;
            if self.on_count >= self.debounce_on && !self.is_present {
                self.is_present = true;
                self.total_detections += 1;
            }
        } else {
            self.off_count += 1;
            self.on_count = 0;
            if self.off_count >= self.debounce_off && self.is_present {
                self.is_present = false;
            }
        }

        // Update baseline when no presence
        if !self.is_present {
            self.baseline_stats.update(signal_variance);
        }

        self.is_present
    }
}

#[derive(serde::Serialize)]
struct PresenceReport {
    present: bool,
    confidence: f64,
    signal_variance: f64,
    baseline_variance: f64,
    signal_to_baseline_ratio: f64,
    total_detections: u64,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_presence(report: &PresenceReport) -> Result<(), String> {
    let vector = vec![
        if report.present { 1.0 } else { 0.0 },
        report.confidence,
        (report.signal_variance / 100.0).min(1.0),
        report.signal_to_baseline_ratio.min(1.0),
        0.0, 0.0, 0.0, 0.0,
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

fn run_once(detector: &mut PresenceDetector) -> Result<PresenceReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let mut stats = WelfordStats::new();
    for ch in samples.iter().take(256) {
        if let Some(val) = ch.get("value").and_then(|v| v.as_f64()) {
            stats.update(val);
        }
    }
    if stats.count < 2 {
        return Err("insufficient sensor data".into());
    }

    let signal_var = stats.variance();
    let present = detector.update(signal_var);
    let baseline_var = detector.baseline_stats.variance().max(1e-10);
    let ratio = signal_var / baseline_var;
    let confidence = if present {
        (ratio - 1.0).min(1.0).max(0.0)
    } else {
        (1.0 - ratio).min(1.0).max(0.0)
    };

    Ok(PresenceReport {
        present,
        confidence,
        signal_variance: signal_var,
        baseline_variance: baseline_var,
        signal_to_baseline_ratio: ratio,
        total_detections: detector.total_detections,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5);

    eprintln!("[cog-presence] starting (interval={}s)", interval);
    let mut detector = PresenceDetector::new(10.0);

    loop {
        let start = Instant::now();
        match run_once(&mut detector) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_presence(&report) {
                    eprintln!("[cog-presence] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-presence] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
