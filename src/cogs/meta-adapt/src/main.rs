//! Cognitum Cog: Meta-Adapt
//!
//! Self-tuning cog. Monitor own detection accuracy (false positive rate).
//! Adjust thresholds automatically using gradient-free optimization
//! (golden section search on threshold parameter).
//!
//! Usage:
//!   cog-meta-adapt --once
//!   cog-meta-adapt --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const PHI: f64 = 1.618033988749895;

/// Golden section search to minimize a function on [a, b]
fn golden_section_search<F: Fn(f64) -> f64>(f: &F, mut a: f64, mut b: f64, tol: f64) -> f64 {
    let mut c = b - (b - a) / PHI;
    let mut d = a + (b - a) / PHI;
    let max_iter = 50;
    let mut iter = 0;

    while (b - a).abs() > tol && iter < max_iter {
        if f(c) < f(d) {
            b = d;
        } else {
            a = c;
        }
        c = b - (b - a) / PHI;
        d = a + (b - a) / PHI;
        iter += 1;
    }
    (a + b) / 2.0
}

/// Online statistics tracker
struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl RunningStats {
    fn new() -> Self {
        Self { count: 0, mean: 0.0, m2: 0.0 }
    }

    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn std_dev(&self) -> f64 {
        if self.count < 2 { return 1.0; }
        (self.m2 / (self.count - 1) as f64).sqrt()
    }
}

/// Evaluate detection performance at a given threshold
/// Returns a cost: weighted sum of false positive rate and miss rate
fn evaluate_threshold(values: &[f64], stats: &RunningStats, threshold: f64) -> f64 {
    let sd = stats.std_dev();
    if sd < 1e-10 {
        return 0.5;
    }

    let mut alerts = 0;
    let mut true_anomalies = 0;
    for &v in values {
        let z = (v - stats.mean).abs() / sd;
        let is_alert = z > threshold;
        // Heuristic: extreme z-scores (>3) are "true" anomalies
        let is_true_anomaly = z > 3.0;
        if is_alert {
            alerts += 1;
        }
        if is_true_anomaly {
            true_anomalies += 1;
        }
    }

    let n = values.len() as f64;
    let alert_rate = alerts as f64 / n;
    let anomaly_rate = true_anomalies as f64 / n;

    // False positive rate estimate
    let fp_rate = if alerts > true_anomalies {
        (alerts - true_anomalies) as f64 / n
    } else {
        0.0
    };

    // Miss rate
    let miss_rate = if true_anomalies > 0 {
        let caught = alerts.min(true_anomalies);
        1.0 - caught as f64 / true_anomalies as f64
    } else {
        0.0
    };

    // Cost: balance false positives vs misses (misses are worse)
    let _ = (alert_rate, anomaly_rate); // suppress warnings
    fp_rate + 2.0 * miss_rate
}

#[derive(serde::Serialize)]
struct AdaptResult {
    optimal_threshold: f64,
    previous_threshold: f64,
    false_positive_rate: f64,
    detection_cost: f64,
    signal_mean: f64,
    signal_std: f64,
    sample_count: usize,
    vector: [f64; 8],
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
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(v: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, v]], "dedup": true });
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

fn run_once(prev_threshold: f64) -> Result<(AdaptResult, f64), String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() {
        return Err("no sensor readings".into());
    }

    let mut stats = RunningStats::new();
    for &v in &values {
        stats.update(v);
    }

    // Optimize threshold using golden section search on [0.5, 4.0]
    let optimal = golden_section_search(
        &|t| evaluate_threshold(&values, &stats, t),
        0.5,
        4.0,
        0.01,
    );

    let cost = evaluate_threshold(&values, &stats, optimal);
    let sd = stats.std_dev();
    let fp_rate = if sd > 1e-10 {
        values.iter().filter(|&&v| {
            let z = (v - stats.mean).abs() / sd;
            z > optimal && z <= 3.0
        }).count() as f64 / values.len() as f64
    } else {
        0.0
    };

    let vector = [
        optimal / 4.0,
        prev_threshold / 4.0,
        fp_rate,
        cost,
        stats.mean,
        stats.std_dev(),
        values.len() as f64 / 100.0,
        (optimal - prev_threshold).abs() / 4.0,
    ];

    let _ = store_vector(&vector);

    Ok((AdaptResult {
        optimal_threshold: optimal,
        previous_threshold: prev_threshold,
        false_positive_rate: fp_rate,
        detection_cost: cost,
        signal_mean: stats.mean,
        signal_std: stats.std_dev(),
        sample_count: values.len(),
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }, optimal))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-meta-adapt] starting (interval={interval}s, once={once})");

    let mut threshold = 2.0; // Initial threshold (z-score)

    loop {
        let start = Instant::now();
        match run_once(threshold) {
            Ok((r, new_t)) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if (new_t - threshold).abs() > 0.3 {
                    eprintln!("[cog-meta-adapt] ALERT: threshold shifted {:.2} -> {:.2}", threshold, new_t);
                }
                threshold = new_t;
            }
            Err(e) => eprintln!("[cog-meta-adapt] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
