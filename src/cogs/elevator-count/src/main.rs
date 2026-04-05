//! Cognitum Cog: Elevator Count
//!
//! Counts people by signal disruption intensity. More people = more signal
//! variance. Uses online linear regression (variance vs known occupancy)
//! to calibrate. Initializes with default scale, learns over time.
//!
//! Usage:
//!   cog-elevator-count --once
//!   cog-elevator-count --interval 5

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

/// Online linear regression: y = slope * x + intercept
/// x = variance, y = people count
struct LinearRegressor {
    n: u64,
    sum_x: f64,
    sum_y: f64,
    sum_xy: f64,
    sum_x2: f64,
    slope: f64,
    intercept: f64,
}

impl LinearRegressor {
    fn new(default_slope: f64, default_intercept: f64) -> Self {
        Self {
            n: 0, sum_x: 0.0, sum_y: 0.0, sum_xy: 0.0, sum_x2: 0.0,
            slope: default_slope, intercept: default_intercept,
        }
    }

    fn add_calibration_point(&mut self, variance: f64, known_count: f64) {
        self.n += 1;
        self.sum_x += variance;
        self.sum_y += known_count;
        self.sum_xy += variance * known_count;
        self.sum_x2 += variance * variance;

        if self.n >= 2 {
            let denom = (self.n as f64) * self.sum_x2 - self.sum_x * self.sum_x;
            if denom.abs() > 1e-10 {
                self.slope = ((self.n as f64) * self.sum_xy - self.sum_x * self.sum_y) / denom;
                self.intercept = (self.sum_y - self.slope * self.sum_x) / self.n as f64;
            }
        }
    }

    fn predict(&self, variance: f64) -> f64 {
        (self.slope * variance + self.intercept).max(0.0)
    }
}

struct ElevatorCounter {
    baseline: WelfordStats,
    regressor: LinearRegressor,
    empty_threshold: f64,
    max_readings: u64,
    peak_count: f64,
}

impl ElevatorCounter {
    fn new() -> Self {
        // Default: ~1 person per 5 units of variance above baseline
        Self {
            baseline: WelfordStats::new(),
            regressor: LinearRegressor::new(0.2, 0.0),
            empty_threshold: 5.0,
            max_readings: 0,
            peak_count: 0.0,
        }
    }

    fn update(&mut self, signal_variance: f64) -> ElevatorReport {
        self.max_readings += 1;
        let baseline_var = self.baseline.variance().max(1e-10);

        // Excess variance above baseline
        let excess = (signal_variance - baseline_var).max(0.0);
        let estimated_count = if excess < self.empty_threshold {
            self.baseline.update(signal_variance);
            0.0
        } else {
            self.regressor.predict(excess)
        };

        let count_rounded = estimated_count.round().max(0.0) as u32;
        if estimated_count > self.peak_count {
            self.peak_count = estimated_count;
        }

        let confidence = if self.regressor.n >= 5 { 0.8 } else { 0.4 };

        ElevatorReport {
            estimated_count: count_rounded,
            raw_estimate: estimated_count,
            signal_variance,
            baseline_variance: baseline_var,
            excess_variance: excess,
            confidence,
            peak_count: self.peak_count.round() as u32,
            calibration_points: self.regressor.n,
            timestamp: now_secs(),
        }
    }
}

#[derive(serde::Serialize)]
struct ElevatorReport {
    estimated_count: u32,
    raw_estimate: f64,
    signal_variance: f64,
    baseline_variance: f64,
    excess_variance: f64,
    confidence: f64,
    peak_count: u32,
    calibration_points: u64,
    timestamp: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

fn store_vector(report: &ElevatorReport) -> Result<(), String> {
    let vector = vec![
        report.estimated_count as f64,
        report.raw_estimate,
        report.signal_variance / 100.0,
        report.excess_variance / 100.0,
        report.confidence,
        report.peak_count as f64,
        report.calibration_points as f64,
        0.0,
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

fn run_once(counter: &mut ElevatorCounter) -> Result<ElevatorReport, String> {
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

    let report = counter.update(stats.variance());

    if report.estimated_count > 8 {
        eprintln!("[cog-elevator-count] ALERT: high occupancy={}", report.estimated_count);
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
        .unwrap_or(5);

    eprintln!("[cog-elevator-count] starting (interval={}s)", interval);
    let mut counter = ElevatorCounter::new();

    loop {
        let start = Instant::now();
        match run_once(&mut counter) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-elevator-count] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-elevator-count] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
