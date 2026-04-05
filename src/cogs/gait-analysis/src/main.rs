//! Cognitum Cog: Gait Analysis
//!
//! Detects walking cadence from periodic signal patterns using autocorrelation
//! to find stride period. Computes symmetry ratio, regularity index, and
//! fall risk score based on cadence variability (Welford stats on stride intervals).
//!
//! Usage:
//!   cog-gait-analysis --once
//!   cog-gait-analysis --interval 10

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
    fn std_dev(&self) -> f64 { self.variance().sqrt() }
    fn cv(&self) -> f64 {
        if self.mean.abs() < 1e-10 { 0.0 } else { self.std_dev() / self.mean.abs() }
    }
}

/// Normalized autocorrelation of signal.
/// Returns autocorrelation values for lags 0..max_lag.
fn autocorrelation(signal: &[f64], max_lag: usize) -> Vec<f64> {
    let n = signal.len();
    if n < 2 { return vec![]; }
    let mean = signal.iter().sum::<f64>() / n as f64;
    let var: f64 = signal.iter().map(|v| (v - mean).powi(2)).sum();
    if var < 1e-15 { return vec![0.0; max_lag]; }

    let mut result = Vec::with_capacity(max_lag);
    for lag in 0..max_lag.min(n) {
        let mut sum = 0.0;
        for i in 0..n - lag {
            sum += (signal[i] - mean) * (signal[i + lag] - mean);
        }
        result.push(sum / var);
    }
    result
}

/// Find the dominant stride period from autocorrelation.
/// Searches for the first significant peak after lag=min_lag.
/// Returns (lag_index, peak_value) or None.
fn find_stride_period(acf: &[f64], min_lag: usize) -> Option<(usize, f64)> {
    if acf.len() < min_lag + 2 { return None; }

    let mut best_lag = 0;
    let mut best_val = 0.0;

    // Find peaks (local maxima) in the autocorrelation
    for i in min_lag.max(1)..acf.len() - 1 {
        if acf[i] > acf[i - 1] && acf[i] > acf[i + 1] && acf[i] > 0.2 {
            if acf[i] > best_val {
                best_val = acf[i];
                best_lag = i;
            }
        }
    }

    if best_lag > 0 { Some((best_lag, best_val)) } else { None }
}

/// Compute symmetry ratio: ratio of first and second stride autocorrelation peaks.
/// Perfect symmetry = 1.0; asymmetric gait < 1.0.
fn symmetry_ratio(acf: &[f64], stride_lag: usize) -> f64 {
    let half_stride = stride_lag / 2;
    if half_stride < 1 || half_stride >= acf.len() || stride_lag >= acf.len() {
        return 0.0;
    }
    // Half-stride peak should correspond to step, full stride to stride
    let step_peak = acf[half_stride].abs();
    let stride_peak = acf[stride_lag].abs();
    if stride_peak < 1e-10 { return 0.0; }
    (step_peak / stride_peak).min(1.0)
}

/// Detect individual stride intervals from zero-crossings of the signal
fn stride_intervals(signal: &[f64], sample_rate: f64) -> Vec<f64> {
    if signal.len() < 4 { return vec![]; }
    let mut crossings = Vec::new();
    for i in 1..signal.len() {
        if signal[i - 1] < 0.0 && signal[i] >= 0.0 {
            crossings.push(i);
        }
    }
    if crossings.len() < 2 { return vec![]; }
    crossings.windows(2)
        .map(|w| (w[1] - w[0]) as f64 / sample_rate)
        .collect()
}

#[derive(serde::Serialize)]
struct GaitReport {
    cadence_steps_per_min: f64,
    stride_period_s: f64,
    regularity_index: f64,
    symmetry_ratio: f64,
    stride_variability_cv: f64,
    fall_risk_score: f64,
    walking_detected: bool,
    alerts: Vec<String>,
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

fn store_report(report: &GaitReport) -> Result<(), String> {
    let vector = vec![
        report.cadence_steps_per_min / 200.0,
        report.stride_period_s,
        report.regularity_index,
        report.symmetry_ratio,
        report.stride_variability_cv,
        report.fall_risk_score,
        if report.walking_detected { 1.0 } else { 0.0 },
        if report.alerts.is_empty() { 0.0 } else { 1.0 },
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

fn run_once() -> Result<GaitReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let amplitudes: Vec<f64> = samples.iter()
        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
        .collect();
    if amplitudes.len() < 10 {
        return Err("insufficient sensor data for gait analysis".into());
    }

    let sample_rate = 10.0;
    let max_lag = amplitudes.len().min(100);
    let acf = autocorrelation(&amplitudes, max_lag);

    // Minimum lag: ~0.3s stride at 10Hz = 3 samples
    let stride_result = find_stride_period(&acf, 3);

    let (walking, stride_lag, regularity) = match stride_result {
        Some((lag, peak_val)) => (true, lag, peak_val),
        None => (false, 0, 0.0),
    };

    let stride_period = stride_lag as f64 / sample_rate;
    let cadence = if stride_period > 0.0 { 60.0 / stride_period * 2.0 } else { 0.0 };
    let sym_ratio = if walking { symmetry_ratio(&acf, stride_lag) } else { 0.0 };

    // Compute stride variability using Welford on detected intervals
    let intervals = stride_intervals(&amplitudes, sample_rate);
    let mut stride_stats = WelfordStats::new();
    for &ivl in &intervals {
        stride_stats.update(ivl);
    }
    let variability_cv = stride_stats.cv();

    // Fall risk: composite of variability, asymmetry, and low regularity
    let fall_risk = if !walking {
        0.0
    } else {
        let var_component = (variability_cv * 2.0).min(1.0);
        let sym_component = (1.0 - sym_ratio).min(1.0);
        let reg_component = (1.0 - regularity).min(1.0);
        ((var_component * 0.4 + sym_component * 0.3 + reg_component * 0.3) * 100.0).min(100.0)
    };

    let mut alerts = Vec::new();
    if walking {
        if fall_risk > 60.0 {
            alerts.push(format!("HIGH_FALL_RISK: score={:.0}%", fall_risk));
        }
        if sym_ratio < 0.5 && sym_ratio > 0.0 {
            alerts.push(format!("GAIT_ASYMMETRY: symmetry={:.2}", sym_ratio));
        }
        if variability_cv > 0.25 {
            alerts.push(format!("IRREGULAR_GAIT: stride_CV={:.3}", variability_cv));
        }
        if cadence > 160.0 {
            alerts.push(format!("HIGH_CADENCE: {:.0} steps/min", cadence));
        }
    }

    Ok(GaitReport {
        cadence_steps_per_min: cadence,
        stride_period_s: stride_period,
        regularity_index: regularity,
        symmetry_ratio: sym_ratio,
        stride_variability_cv: variability_cv,
        fall_risk_score: fall_risk,
        walking_detected: walking,
        alerts,
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
        .unwrap_or(10);

    eprintln!("[cog-gait-analysis] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_report(&report) {
                    eprintln!("[cog-gait-analysis] store error: {e}");
                }
                if !report.alerts.is_empty() {
                    eprintln!("[cog-gait-analysis] ALERT: {:?}", report.alerts);
                }
            }
            Err(e) => eprintln!("[cog-gait-analysis] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
