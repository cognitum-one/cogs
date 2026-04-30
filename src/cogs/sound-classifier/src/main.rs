//! Cognitum Cog: Sound Classifier
//!
//! Classifies environmental sounds from signal modulation patterns.
//! Detects characteristic frequencies: glass break (high-freq burst),
//! alarm (periodic 1-3Hz), baby cry (irregular high-pitch).
//!
//! Usage:
//!   cog-sound-classifier --once
//!   cog-sound-classifier --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

/// Feature: high-frequency energy ratio (proportion of energy in high-freq diffs)
fn high_freq_energy_ratio(values: &[f64]) -> f64 {
    if values.len() < 4 { return 0.0; }
    let total_energy: f64 = values.iter().map(|v| v * v).sum();
    if total_energy < 1e-10 { return 0.0; }

    // Second-order differences approximate high-freq content
    let hf_energy: f64 = values.windows(3)
        .map(|w| (w[2] - 2.0 * w[1] + w[0]).powi(2))
        .sum();
    (hf_energy / total_energy).min(1.0)
}

/// Feature: burst detection (sudden amplitude spike)
fn burst_score(values: &[f64]) -> f64 {
    if values.len() < 3 { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let std_dev = (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt();
    if std_dev < 1e-10 { return 0.0; }

    // Kurtosis: high kurtosis = impulsive (burst-like)
    let m4: f64 = values.iter().map(|v| ((v - mean) / std_dev).powi(4)).sum::<f64>() / values.len() as f64;
    // Normal kurtosis = 3, subtract to get excess kurtosis
    ((m4 - 3.0) / 10.0).max(0.0).min(1.0)
}

/// Feature: periodic modulation (alarm-like)
fn periodic_modulation(values: &[f64]) -> f64 {
    if values.len() < 6 { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    if var < 1e-10 { return 0.0; }

    // Check autocorrelation at alarm-like lags (1-3 Hz = every 3-8 samples at ~10Hz)
    let mut best_corr: f64 = 0.0;
    for lag in 3..values.len().min(10) {
        let n = values.len() - lag;
        let cov: f64 = (0..n).map(|i| (values[i] - mean) * (values[i + lag] - mean)).sum();
        let corr = cov / var;
        if corr > best_corr { best_corr = corr; }
    }
    best_corr.max(0.0)
}

/// Feature: irregularity (cry-like) — high variance of inter-sample differences
fn irregularity_score(values: &[f64]) -> f64 {
    if values.len() < 3 { return 0.0; }
    let diffs: Vec<f64> = values.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let mean_diff = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let var_diff = diffs.iter().map(|d| (d - mean_diff).powi(2)).sum::<f64>() / diffs.len() as f64;
    // High variance of diffs = irregular
    (var_diff.sqrt() / (mean_diff.abs() + 1e-6)).tanh()
}

/// Feature: amplitude envelope
fn amplitude_level(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let rms = (values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64).sqrt();
    rms
}

#[derive(serde::Serialize)]
struct SoundReport {
    classification: String,
    confidence: f64,
    scores: SoundScores,
    amplitude_rms: f64,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct SoundScores {
    glass_break: f64,
    alarm: f64,
    baby_cry: f64,
    ambient: f64,
}

fn classify_sound(scores: &SoundScores) -> (&'static str, f64) {
    let candidates = [
        ("glass-break", scores.glass_break),
        ("alarm", scores.alarm),
        ("baby-cry", scores.baby_cry),
        ("ambient", scores.ambient),
    ];
    let best = candidates.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
    (best.0, best.1)
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

fn run_once() -> Result<SoundReport, String> {
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

    let hf_ratio = high_freq_energy_ratio(&values);
    let burst = burst_score(&values);
    let periodic = periodic_modulation(&values);
    let irregular = irregularity_score(&values);
    let amp = amplitude_level(&values);

    // Glass break: high-freq + burst + high amplitude
    let glass = (hf_ratio * 0.4 + burst * 0.4 + (amp / 100.0).min(1.0) * 0.2).min(1.0);
    // Alarm: periodic + moderate amplitude
    let alarm = (periodic * 0.6 + (1.0 - burst) * 0.2 + (amp / 50.0).min(1.0) * 0.2).min(1.0);
    // Baby cry: irregular + high-freq + moderate amplitude
    let cry = (irregular * 0.4 + hf_ratio * 0.3 + (amp / 80.0).min(1.0) * 0.3).min(1.0);
    // Ambient: inverse of everything else
    let ambient = (1.0 - glass.max(alarm).max(cry)).max(0.0);

    let scores = SoundScores { glass_break: glass, alarm, baby_cry: cry, ambient };
    let (label, confidence) = classify_sound(&scores);

    let report = SoundReport {
        classification: label.into(),
        confidence,
        scores,
        amplitude_rms: amp,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        glass, alarm, cry, ambient,
        amp / 100.0, hf_ratio, burst, periodic,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-sound-classifier] store error: {e}");
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

    eprintln!("[cog-sound-classifier] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.classification != "ambient" && report.confidence > 0.5 {
                    eprintln!("[cog-sound-classifier] ALERT: {} detected (conf={:.2})",
                        report.classification, report.confidence);
                }
            }
            Err(e) => eprintln!("[cog-sound-classifier] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
