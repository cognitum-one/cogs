//! Cognitum Cog: Structural Vibration Detection
//!
//! Detects dangerous mechanical vibrations using a simple high-pass filter
//! (>10Hz proxy via first-difference) and RMS energy computation.
//! Alerts on threshold exceedance or resonance patterns.
//!
//! Usage:
//!   cog-structural-vibration --once
//!   cog-structural-vibration --interval 1 --threshold 50.0

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const DEFAULT_RMS_THRESHOLD: f64 = 50.0;
const RESONANCE_RATIO: f64 = 3.0;  // Peak-to-mean ratio indicating resonance

struct VibrationState {
    signal_history: Vec<Vec<f64>>,  // History of multi-channel snapshots
    max_history: usize,
    baseline_rms: f64,
    baseline_count: u64,
}

impl VibrationState {
    fn new() -> Self {
        Self {
            signal_history: Vec::new(),
            max_history: 30,
            baseline_rms: 0.0,
            baseline_count: 0,
        }
    }

    fn update_baseline(&mut self, rms: f64) {
        self.baseline_count += 1;
        self.baseline_rms += (rms - self.baseline_rms) / self.baseline_count as f64;
    }
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(vec: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec.to_vec()]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

/// High-pass filter via first-difference (removes DC and low-freq)
fn high_pass_diff(current: &[f64], previous: &[f64]) -> Vec<f64> {
    current.iter().zip(previous.iter())
        .map(|(c, p)| c - p)
        .collect()
}

/// RMS energy of a signal vector
fn rms(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let sum_sq: f64 = values.iter().map(|v| v * v).sum();
    (sum_sq / values.len() as f64).sqrt()
}

/// Detect resonance: check if peak frequency dominates (peak >> mean)
fn detect_resonance(history: &[Vec<f64>]) -> (bool, f64) {
    if history.len() < 5 { return (false, 0.0); }

    // Compute RMS for each time step
    let rms_series: Vec<f64> = history.iter()
        .map(|snapshot| rms(snapshot))
        .collect();

    let mean_rms: f64 = rms_series.iter().sum::<f64>() / rms_series.len() as f64;
    let peak_rms: f64 = rms_series.iter().cloned().fold(0.0f64, f64::max);

    let ratio = if mean_rms > 0.01 { peak_rms / mean_rms } else { 0.0 };
    (ratio > RESONANCE_RATIO, ratio)
}

#[derive(serde::Serialize)]
struct VibrationReport {
    rms_energy: f64,
    baseline_rms: f64,
    rms_ratio: f64,
    peak_channel_value: f64,
    high_pass_rms: f64,
    resonance_detected: bool,
    resonance_ratio: f64,
    threshold_exceeded: bool,
    alert: bool,
    alert_reason: Option<String>,
    timestamp: u64,
}

fn run_once(state: &mut VibrationState, threshold: f64) -> Result<VibrationReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    if values.is_empty() {
        return Err("no sensor data".into());
    }

    // Compute high-pass filtered signal
    let hp_values = if let Some(prev) = state.signal_history.last() {
        high_pass_diff(&values, prev)
    } else {
        values.clone()
    };

    // Store current snapshot
    state.signal_history.push(values.clone());
    if state.signal_history.len() > state.max_history {
        state.signal_history.remove(0);
    }

    let current_rms = rms(&values);
    let hp_rms = rms(&hp_values);
    let peak = values.iter().cloned().fold(0.0f64, |a, b| a.max(b.abs()));

    let rms_ratio = if state.baseline_rms > 0.01 {
        current_rms / state.baseline_rms
    } else { 1.0 };

    let threshold_exceeded = hp_rms > threshold;
    let (resonance, resonance_ratio) = detect_resonance(&state.signal_history);

    // Update baseline when not alarming
    if !threshold_exceeded && !resonance {
        state.update_baseline(current_rms);
    }

    let alert = threshold_exceeded || resonance;
    let alert_reason = if threshold_exceeded && resonance {
        Some(format!("Dangerous vibration: RMS={hp_rms:.1} + resonance (ratio={resonance_ratio:.1})"))
    } else if threshold_exceeded {
        Some(format!("Vibration threshold exceeded: RMS={hp_rms:.1} > {threshold:.1}"))
    } else if resonance {
        Some(format!("Resonance pattern detected: ratio={resonance_ratio:.1}"))
    } else { None };

    let vector = [
        (hp_rms / threshold).min(2.0) / 2.0,
        (current_rms / 100.0).min(1.0),
        rms_ratio.min(10.0) / 10.0,
        (peak / 200.0).min(1.0),
        if resonance { 1.0 } else { 0.0 },
        resonance_ratio.min(10.0) / 10.0,
        if threshold_exceeded { 1.0 } else { 0.0 },
        if alert { 1.0 } else { 0.0 },
    ];
    let _ = store_vector(&vector);

    Ok(VibrationReport {
        rms_energy: current_rms,
        baseline_rms: state.baseline_rms,
        rms_ratio,
        peak_channel_value: peak,
        high_pass_rms: hp_rms,
        resonance_detected: resonance,
        resonance_ratio,
        threshold_exceeded,
        alert,
        alert_reason,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    let threshold = args.iter()
        .position(|a| a == "--threshold")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_RMS_THRESHOLD);

    eprintln!("[cog-structural-vibration] starting (threshold={threshold}, interval={interval}s)");
    let mut state = VibrationState::new();

    loop {
        let start = Instant::now();
        match run_once(&mut state, threshold) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.alert {
                    eprintln!("[cog-structural-vibration] ALERT: {}", report.alert_reason.as_deref().unwrap_or("unknown"));
                }
            }
            Err(e) => eprintln!("[cog-structural-vibration] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
