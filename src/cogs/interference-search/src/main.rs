//! Cognitum Cog: Interference Search
//!
//! Grover-inspired search. Amplitude amplification on signal features
//! to rapidly find anomalies in large feature spaces. Quadratic speedup
//! proxy via importance sampling.
//!
//! Usage:
//!   cog-interference-search --once
//!   cog-interference-search --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Grover-inspired amplitude amplification
/// Amplifies "marked" items (anomalous features) relative to unmarked
struct GroverSearch {
    /// Feature amplitudes (normalized)
    amplitudes: Vec<f64>,
    /// Oracle: marks features as anomalous (true = marked)
    marked: Vec<bool>,
}

impl GroverSearch {
    fn new(features: &[f64], threshold: f64) -> Self {
        let n = features.len();
        // Initialize uniform superposition
        let amp = 1.0 / (n as f64).sqrt();
        let amplitudes = vec![amp; n];

        // Oracle marks anomalous features
        let mean = features.iter().sum::<f64>() / n.max(1) as f64;
        let var = features.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n.max(1) as f64;
        let sd = var.sqrt();

        let marked: Vec<bool> = features.iter().map(|v| {
            if sd > 1e-10 {
                ((v - mean) / sd).abs() > threshold
            } else {
                false
            }
        }).collect();

        Self { amplitudes, marked }
    }

    /// Number of optimal Grover iterations: ~pi/4 * sqrt(N/M)
    fn optimal_iterations(&self) -> usize {
        let n = self.amplitudes.len() as f64;
        let m = self.marked.iter().filter(|&&b| b).count() as f64;
        if m < 1.0 || n < 1.0 { return 1; }
        let iters = (std::f64::consts::PI / 4.0 * (n / m).sqrt()) as usize;
        iters.max(1).min(20)
    }

    /// Run Grover iterations
    fn search(&mut self) -> Vec<(usize, f64)> {
        let iterations = self.optimal_iterations();

        for _ in 0..iterations {
            // Phase oracle: flip sign of marked states
            for (i, &is_marked) in self.marked.iter().enumerate() {
                if is_marked {
                    self.amplitudes[i] = -self.amplitudes[i];
                }
            }

            // Diffusion operator (inversion about mean)
            let mean: f64 = self.amplitudes.iter().sum::<f64>() / self.amplitudes.len() as f64;
            for a in &mut self.amplitudes {
                *a = 2.0 * mean - *a;
            }
        }

        // Return indices sorted by amplitude (probability)
        let mut results: Vec<(usize, f64)> = self.amplitudes.iter().enumerate()
            .map(|(i, &a)| (i, a * a)) // Convert to probability
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

/// Importance sampling as classical proxy for amplitude amplification
fn importance_sample(features: &[f64], threshold: f64) -> Vec<(usize, f64)> {
    let mean = features.iter().sum::<f64>() / features.len().max(1) as f64;
    let var = features.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / features.len().max(1) as f64;
    let sd = var.sqrt().max(1e-10);

    let mut scores: Vec<(usize, f64)> = features.iter().enumerate().map(|(i, &v)| {
        let z = ((v - mean) / sd).abs();
        let importance = if z > threshold { z * z } else { 0.0 };
        (i, importance)
    }).collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

#[derive(serde::Serialize)]
struct SearchResult {
    total_features: usize,
    marked_anomalies: usize,
    grover_iterations: usize,
    top_anomalies: Vec<AnomalyHit>,
    amplification_factor: f64,
    classical_comparison: Vec<AnomalyHit>,
    speedup_estimate: f64,
    vector: [f64; DIM],
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct AnomalyHit {
    index: usize,
    probability: f64,
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

fn store_vector(v: &[f64; DIM]) -> Result<(), String> {
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

fn run_once() -> Result<SearchResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    let threshold = 2.0;

    // Grover-inspired search
    let mut grover = GroverSearch::new(&values, threshold);
    let iterations = grover.optimal_iterations();
    let grover_results = grover.search();
    let marked = grover.marked.iter().filter(|&&b| b).count();

    let top_grover: Vec<AnomalyHit> = grover_results.iter().take(5)
        .map(|&(idx, prob)| AnomalyHit { index: idx, probability: prob })
        .collect();

    // Classical comparison
    let classical = importance_sample(&values, threshold);
    let top_classical: Vec<AnomalyHit> = classical.iter().take(5)
        .map(|&(idx, score)| AnomalyHit { index: idx, probability: score })
        .collect();

    // Amplification: ratio of anomaly probability (Grover vs uniform)
    let uniform_prob = 1.0 / values.len().max(1) as f64;
    let top_prob = top_grover.first().map(|h| h.probability).unwrap_or(uniform_prob);
    let amplification = top_prob / uniform_prob;

    // Theoretical speedup: sqrt(N/M) for M marked out of N
    let speedup = if marked > 0 {
        (values.len() as f64 / marked as f64).sqrt()
    } else {
        1.0
    };

    let vector = [
        marked as f64 / values.len().max(1) as f64,
        amplification.min(10.0) / 10.0,
        speedup.min(10.0) / 10.0,
        iterations as f64 / 20.0,
        top_prob,
        values.len() as f64 / 100.0,
        threshold / 4.0,
        if marked > 0 { 1.0 } else { 0.0 },
    ];

    let _ = store_vector(&vector);

    Ok(SearchResult {
        total_features: values.len(),
        marked_anomalies: marked,
        grover_iterations: iterations,
        top_anomalies: top_grover,
        amplification_factor: amplification,
        classical_comparison: top_classical,
        speedup_estimate: speedup,
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-interference-search] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if r.marked_anomalies > 0 {
                    eprintln!("[cog-interference-search] ALERT: {} anomalies amplified ({}x)", r.marked_anomalies, r.amplification_factor as u32);
                }
            }
            Err(e) => eprintln!("[cog-interference-search] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
