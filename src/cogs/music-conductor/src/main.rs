//! Cognitum Cog: Music Conductor
//!
//! Extracts tempo from periodic arm gestures using autocorrelation.
//! Reports BPM, beat confidence, and dynamics (amplitude envelope).
//!
//! Usage:
//!   cog-music-conductor --once
//!   cog-music-conductor --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

/// Compute normalized autocorrelation at a given lag
fn autocorrelation(signal: &[f64], lag: usize) -> f64 {
    if lag >= signal.len() { return 0.0; }
    let n = signal.len() - lag;
    if n == 0 { return 0.0; }
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let var: f64 = signal.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
    if var < 1e-10 { return 0.0; }
    let cov: f64 = (0..n).map(|i| (signal[i] - mean) * (signal[i + lag] - mean)).sum();
    cov / var
}

/// Find peak autocorrelation lag (beat period in samples)
fn find_beat_period(signal: &[f64], min_lag: usize, max_lag: usize) -> (usize, f64) {
    let mut best_lag = min_lag;
    let mut best_corr: f64 = -1.0;
    for lag in min_lag..=max_lag.min(signal.len() / 2) {
        let corr = autocorrelation(signal, lag);
        if corr > best_corr {
            best_corr = corr;
            best_lag = lag;
        }
    }
    (best_lag, best_corr)
}

/// Compute amplitude envelope (RMS in windows)
fn amplitude_dynamics(values: &[f64], window: usize) -> (f64, f64, f64) {
    if values.is_empty() { return (0.0, 0.0, 0.0); }
    let mut rms_values = Vec::new();
    for chunk in values.chunks(window.max(1)) {
        let rms = (chunk.iter().map(|v| v * v).sum::<f64>() / chunk.len() as f64).sqrt();
        rms_values.push(rms);
    }
    let mean = rms_values.iter().sum::<f64>() / rms_values.len() as f64;
    let min = rms_values.iter().cloned().fold(f64::MAX, f64::min);
    let max = rms_values.iter().cloned().fold(f64::MIN, f64::max);
    (mean, min, max)
}

#[derive(serde::Serialize)]
struct ConductorReport {
    bpm: f64,
    beat_period_samples: usize,
    beat_confidence: f64,
    dynamics_mean: f64,
    dynamics_min: f64,
    dynamics_max: f64,
    dynamic_range_db: f64,
    tempo_class: String,
    timestamp: u64,
}

fn classify_tempo(bpm: f64) -> &'static str {
    if bpm < 60.0 { "largo" }
    else if bpm < 80.0 { "adagio" }
    else if bpm < 100.0 { "andante" }
    else if bpm < 120.0 { "moderato" }
    else if bpm < 140.0 { "allegro" }
    else if bpm < 180.0 { "vivace" }
    else { "presto" }
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

fn run_once(sample_rate: f64) -> Result<ConductorReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples in sensor data")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    if values.len() < 4 {
        return Err("insufficient samples for beat detection".into());
    }

    // BPM range: 40-200 => period in samples
    let min_lag = (sample_rate * 60.0 / 200.0).max(1.0) as usize;
    let max_lag = (sample_rate * 60.0 / 40.0) as usize;

    let (period, confidence) = find_beat_period(&values, min_lag, max_lag);
    let bpm = if period > 0 { sample_rate * 60.0 / period as f64 } else { 0.0 };

    let (dyn_mean, dyn_min, dyn_max) = amplitude_dynamics(&values, 4);
    let dynamic_range_db = if dyn_min > 1e-10 { 20.0 * (dyn_max / dyn_min).log10() } else { 0.0 };

    let report = ConductorReport {
        bpm,
        beat_period_samples: period,
        beat_confidence: confidence.max(0.0),
        dynamics_mean: dyn_mean,
        dynamics_min: dyn_min,
        dynamics_max: dyn_max,
        dynamic_range_db,
        tempo_class: classify_tempo(bpm).into(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        bpm / 200.0,
        confidence.max(0.0),
        dyn_mean / 100.0,
        dynamic_range_db / 60.0,
        period as f64 / 100.0,
        dyn_min / 100.0,
        dyn_max / 100.0,
        0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-music-conductor] store error: {e}");
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
    let sample_rate = args.iter()
        .position(|a| a == "--sample-rate")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(100.0);

    eprintln!("[cog-music-conductor] starting (interval={}s, sample_rate={:.0}Hz)", interval, sample_rate);

    loop {
        let start = Instant::now();
        match run_once(sample_rate) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.beat_confidence > 0.5 {
                    eprintln!("[cog-music-conductor] ALERT: tempo={:.0} BPM ({}) conf={:.2}",
                        report.bpm, report.tempo_class, report.beat_confidence);
                }
            }
            Err(e) => eprintln!("[cog-music-conductor] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
