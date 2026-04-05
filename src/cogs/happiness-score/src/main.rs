//! Cognitum Cog: Happiness Score
//!
//! Composite well-being score from movement patterns, breathing regularity,
//! and activity level. Weighted average of normalized metrics on a 0-100 scale.
//!
//! Usage:
//!   cog-happiness-score --once
//!   cog-happiness-score --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

/// Measure movement quality: moderate, rhythmic movement scores high
fn movement_score(values: &[f64]) -> f64 {
    if values.len() < 2 { return 50.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let std_dev = (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt();

    // Moderate variance is ideal (not frozen, not chaotic)
    // Bell curve centered around optimal_std
    let optimal_std = 15.0;
    let spread = 20.0;
    let deviation = (std_dev - optimal_std).abs() / spread;
    ((-deviation.powi(2)).exp() * 100.0).min(100.0)
}

/// Measure breathing regularity from signal periodicity
fn breathing_regularity(values: &[f64]) -> f64 {
    if values.len() < 6 { return 50.0; }

    // Check autocorrelation at breathing-rate lags (12-20 bpm = lag 3-5 at 1Hz)
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    if var < 1e-10 { return 50.0; }

    let mut best_corr: f64 = 0.0;
    for lag in 2..values.len() / 2 {
        let n = values.len() - lag;
        let cov: f64 = (0..n).map(|i| (values[i] - mean) * (values[i + lag] - mean)).sum();
        let corr = cov / var;
        if corr > best_corr { best_corr = corr; }
    }

    // High regularity (strong autocorrelation) = high score
    (best_corr.max(0.0) * 100.0).min(100.0)
}

/// Measure activity level: not too sedentary, not too frantic
fn activity_score(values: &[f64]) -> f64 {
    if values.is_empty() { return 50.0; }

    // Activity = mean absolute deviation from zero
    let activity = values.iter().map(|v| v.abs()).sum::<f64>() / values.len() as f64;

    // Optimal activity range is moderate
    let optimal = 30.0;
    let spread = 40.0;
    let deviation = (activity - optimal).abs() / spread;
    ((-deviation.powi(2)).exp() * 100.0).min(100.0)
}

/// Check signal diversity (variety of amplitude levels)
fn diversity_score(values: &[f64]) -> f64 {
    if values.len() < 2 { return 50.0; }
    let min = values.iter().cloned().fold(f64::MAX, f64::min);
    let max = values.iter().cloned().fold(f64::MIN, f64::max);
    let range = max - min;
    // Some diversity is good, map 0-100 range to score
    ((range / 100.0).tanh() * 100.0).min(100.0)
}

#[derive(serde::Serialize)]
struct HappinessReport {
    score: f64,
    movement: f64,
    breathing: f64,
    activity: f64,
    diversity: f64,
    level: String,
    timestamp: u64,
}

fn classify_happiness(score: f64) -> &'static str {
    if score < 20.0 { "distressed" }
    else if score < 40.0 { "low" }
    else if score < 60.0 { "neutral" }
    else if score < 80.0 { "good" }
    else { "thriving" }
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

fn run_once() -> Result<HappinessReport, String> {
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

    let movement = movement_score(&values);
    let breathing = breathing_regularity(&values);
    let activity = activity_score(&values);
    let diversity = diversity_score(&values);

    // Weighted composite: movement 30%, breathing 30%, activity 25%, diversity 15%
    let score = movement * 0.30 + breathing * 0.30 + activity * 0.25 + diversity * 0.15;

    let report = HappinessReport {
        score,
        movement,
        breathing,
        activity,
        diversity,
        level: classify_happiness(score).into(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        score / 100.0,
        movement / 100.0,
        breathing / 100.0,
        activity / 100.0,
        diversity / 100.0,
        0.0, 0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-happiness-score] store error: {e}");
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
        .unwrap_or(10);

    eprintln!("[cog-happiness-score] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.score < 30.0 {
                    eprintln!("[cog-happiness-score] ALERT: low well-being score {:.0} ({})",
                        report.score, report.level);
                }
            }
            Err(e) => eprintln!("[cog-happiness-score] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
