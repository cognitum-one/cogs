//! Cognitum Cog: Anomaly Attractor
//!
//! Strange attractor analysis. Embed signal in delay coordinates
//! (x(t), x(t-tau), x(t-2*tau)). Compute Lyapunov exponent estimate
//! via neighbor divergence rate. Detect chaos vs noise.
//!
//! Usage:
//!   cog-anomaly-attractor --once
//!   cog-anomaly-attractor --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

/// Create delay embedding from a time series
/// Returns vectors of dimension `dim` with delay `tau`
fn delay_embed(signal: &[f64], dim: usize, tau: usize) -> Vec<Vec<f64>> {
    let required = (dim - 1) * tau;
    if signal.len() <= required {
        return Vec::new();
    }
    let n = signal.len() - required;
    (0..n)
        .map(|i| (0..dim).map(|d| signal[i + d * tau]).collect())
        .collect()
}

/// Euclidean distance between two vectors
fn euclidean(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Estimate largest Lyapunov exponent via neighbor divergence
/// Positive => chaos, near-zero => periodic, negative => stable
fn estimate_lyapunov(embedded: &[Vec<f64>], steps: usize) -> f64 {
    if embedded.len() < steps + 2 {
        return 0.0;
    }
    let n = embedded.len();
    let mut total_log_divergence = 0.0;
    let mut count = 0u64;
    let min_sep = 2; // temporal separation to avoid autocorrelation

    for i in 0..n.saturating_sub(steps) {
        // Find nearest neighbor with temporal separation
        let mut best_dist = f64::MAX;
        let mut best_j = 0;
        for j in 0..n.saturating_sub(steps) {
            if (i as isize - j as isize).unsigned_abs() < min_sep {
                continue;
            }
            let d = euclidean(&embedded[i], &embedded[j]);
            if d < best_dist && d > 1e-10 {
                best_dist = d;
                best_j = j;
            }
        }
        if best_dist == f64::MAX {
            continue;
        }

        // Measure divergence after `steps` iterations
        let future_dist = euclidean(&embedded[i + steps], &embedded[best_j + steps]);
        if future_dist > 1e-10 && best_dist > 1e-10 {
            total_log_divergence += (future_dist / best_dist).ln();
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }
    total_log_divergence / (count as f64 * steps as f64)
}

/// Estimate correlation dimension (proxy for attractor complexity)
fn correlation_dimension(embedded: &[Vec<f64>]) -> f64 {
    let n = embedded.len();
    if n < 10 {
        return 0.0;
    }
    // Compute pairwise distances
    let mut dists = Vec::new();
    let sample_limit = n.min(100); // limit for O(n^2) computation
    for i in 0..sample_limit {
        for j in (i + 1)..sample_limit {
            dists.push(euclidean(&embedded[i], &embedded[j]));
        }
    }
    if dists.is_empty() {
        return 0.0;
    }
    dists.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Correlation integral at two radii
    let r1 = dists[dists.len() / 4];
    let r2 = dists[dists.len() / 2];
    if r1 < 1e-10 || r2 < 1e-10 || (r2 / r1).ln().abs() < 1e-10 {
        return 0.0;
    }

    let c1 = dists.iter().filter(|&&d| d < r1).count() as f64 / dists.len() as f64;
    let c2 = dists.iter().filter(|&&d| d < r2).count() as f64 / dists.len() as f64;

    if c1 < 1e-10 || c2 < 1e-10 {
        return 0.0;
    }
    (c2 / c1).ln() / (r2 / r1).ln()
}

#[derive(serde::Serialize)]
struct AttractorResult {
    lyapunov_exponent: f64,
    correlation_dimension: f64,
    classification: String,
    embedding_points: usize,
    embedding_dim: usize,
    delay_tau: usize,
    anomalies: Vec<String>,
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

fn run_once() -> Result<AttractorResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let signal: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if signal.len() < 10 {
        return Err("insufficient sensor data".into());
    }

    let dim = 3;
    let tau = 2;
    let embedded = delay_embed(&signal, dim, tau);
    if embedded.is_empty() {
        return Err("signal too short for embedding".into());
    }

    let lyap = estimate_lyapunov(&embedded, 3);
    let corr_dim = correlation_dimension(&embedded);

    let classification = if lyap > 0.1 {
        "chaotic"
    } else if lyap > -0.05 {
        "quasi-periodic"
    } else if lyap < -0.1 {
        "stable-fixed-point"
    } else {
        "periodic"
    };

    let mut anomalies = Vec::new();
    if lyap > 0.5 {
        anomalies.push("HIGH_CHAOS: Lyapunov exponent strongly positive".into());
    }
    if corr_dim > 2.5 {
        anomalies.push(format!("HIGH_COMPLEXITY: correlation dimension={corr_dim:.2}"));
    }

    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let var = signal.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / signal.len() as f64;

    let vector = [
        lyap,
        corr_dim,
        mean,
        var.sqrt(),
        embedded.len() as f64 / 100.0,
        if classification == "chaotic" { 1.0 } else { 0.0 },
        dim as f64 / 10.0,
        tau as f64 / 10.0,
    ];

    let _ = store_vector(&vector);

    Ok(AttractorResult {
        lyapunov_exponent: lyap,
        correlation_dimension: corr_dim,
        classification: classification.into(),
        embedding_points: embedded.len(),
        embedding_dim: dim,
        delay_tau: tau,
        anomalies,
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-anomaly-attractor] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-anomaly-attractor] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-anomaly-attractor] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
