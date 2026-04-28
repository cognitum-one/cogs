//! Cognitum Cog: Optimal Transport
//!
//! Measures movement using Wasserstein distance (earth mover's distance)
//! between signal distributions at different times. More robust than
//! Euclidean for comparing distribution shapes.
//!
//! Usage:
//!   cog-optimal-transport --once
//!   cog-optimal-transport --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const NUM_BINS: usize = 16;
const MAX_HISTORY: usize = 50;

/// Compute 1D Wasserstein distance (earth mover's distance) between two histograms
/// For 1D distributions, W1 = integral of |CDF_a - CDF_b|
fn wasserstein_1d(a: &[f64; NUM_BINS], b: &[f64; NUM_BINS]) -> f64 {
    // Normalize to probability distributions
    let sum_a: f64 = a.iter().sum();
    let sum_b: f64 = b.iter().sum();
    if sum_a < 1e-10 || sum_b < 1e-10 { return 0.0; }

    let pa: Vec<f64> = a.iter().map(|v| v / sum_a).collect();
    let pb: Vec<f64> = b.iter().map(|v| v / sum_b).collect();

    // CDF difference integral
    let mut cdf_a = 0.0;
    let mut cdf_b = 0.0;
    let mut distance = 0.0;

    for i in 0..NUM_BINS {
        cdf_a += pa[i];
        cdf_b += pb[i];
        distance += (cdf_a - cdf_b).abs();
    }

    distance / NUM_BINS as f64
}

/// Build histogram from values
fn build_histogram(values: &[f64]) -> [f64; NUM_BINS] {
    let mut hist = [0.0_f64; NUM_BINS];
    if values.is_empty() { return hist; }

    let min = values.iter().cloned().fold(f64::MAX, f64::min);
    let max = values.iter().cloned().fold(f64::MIN, f64::max);
    let range = max - min;
    if range < 1e-10 {
        hist[NUM_BINS / 2] = values.len() as f64;
        return hist;
    }

    for &v in values {
        let idx = ((v - min) / range * (NUM_BINS - 1) as f64) as usize;
        hist[idx.min(NUM_BINS - 1)] += 1.0;
    }
    hist
}

/// Compute 2D Wasserstein approximation using sliced approach
/// (Average of 1D Wasserstein over random projections)
fn sliced_wasserstein(hist_a: &[f64; NUM_BINS], hist_b: &[f64; NUM_BINS]) -> f64 {
    // For 1D histograms, this is just the regular Wasserstein
    wasserstein_1d(hist_a, hist_b)
}

struct TransportHistory {
    histograms: Vec<[f64; NUM_BINS]>,
    raw_values: Vec<Vec<f64>>,
}

impl TransportHistory {
    fn new() -> Self { Self { histograms: Vec::new(), raw_values: Vec::new() } }

    fn push(&mut self, hist: [f64; NUM_BINS], values: Vec<f64>) {
        self.histograms.push(hist);
        self.raw_values.push(values);
        if self.histograms.len() > MAX_HISTORY {
            self.histograms.remove(0);
            self.raw_values.remove(0);
        }
    }

    fn len(&self) -> usize { self.histograms.len() }
}

#[derive(serde::Serialize)]
struct TransportReport {
    wasserstein_to_prev: f64,
    wasserstein_to_baseline: f64,
    mean_transport: f64,
    max_transport: f64,
    distribution_entropy: f64,
    movement_class: String,
    history_size: usize,
    timestamp: u64,
}

fn distribution_entropy(hist: &[f64; NUM_BINS]) -> f64 {
    let total: f64 = hist.iter().sum();
    if total < 1e-10 { return 0.0; }
    let mut entropy = 0.0;
    for &count in hist {
        if count > 0.0 {
            let p = count / total;
            entropy -= p * p.ln();
        }
    }
    entropy
}

fn classify_movement(w_dist: f64) -> &'static str {
    if w_dist < 0.02 { "stationary" }
    else if w_dist < 0.1 { "subtle" }
    else if w_dist < 0.3 { "moderate" }
    else if w_dist < 0.5 { "active" }
    else { "dramatic" }
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
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

fn run_once(history: &mut TransportHistory) -> Result<TransportReport, String> {
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

    let hist = build_histogram(&values);
    let entropy = distribution_entropy(&hist);

    // Distance to previous
    let w_prev = if let Some(prev) = history.histograms.last() {
        wasserstein_1d(prev, &hist)
    } else { 0.0 };

    // Distance to baseline (first histogram)
    let w_baseline = if let Some(first) = history.histograms.first() {
        wasserstein_1d(first, &hist)
    } else { 0.0 };

    // Mean and max distance to all history
    let distances: Vec<f64> = history.histograms.iter()
        .map(|h| wasserstein_1d(h, &hist))
        .collect();

    let mean_transport = if distances.is_empty() { 0.0 } else {
        distances.iter().sum::<f64>() / distances.len() as f64
    };
    let max_transport = distances.iter().cloned().fold(0.0_f64, f64::max);

    history.push(hist, values);

    let report = TransportReport {
        wasserstein_to_prev: w_prev,
        wasserstein_to_baseline: w_baseline,
        mean_transport,
        max_transport,
        distribution_entropy: entropy,
        movement_class: classify_movement(w_prev).into(),
        history_size: history.len(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        w_prev,
        w_baseline,
        mean_transport,
        max_transport,
        entropy / 3.0,    // normalize (max entropy ~ln(16) ~2.77)
        history.len() as f64 / MAX_HISTORY as f64,
        0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-optimal-transport] store error: {e}");
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

    eprintln!("[cog-optimal-transport] starting (interval={}s)", interval);

    let mut history = TransportHistory::new();

    loop {
        let start = Instant::now();
        match run_once(&mut history) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.wasserstein_to_prev > 0.3 {
                    eprintln!("[cog-optimal-transport] ALERT: {} distribution shift (W={:.3})",
                        report.movement_class, report.wasserstein_to_prev);
                }
            }
            Err(e) => eprintln!("[cog-optimal-transport] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
