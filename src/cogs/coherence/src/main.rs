//! Cognitum Cog: Coherence
//!
//! Signal quality monitor. Compute coherence metrics across all channels.
//! Report overall signal health, per-channel quality scores, and noise
//! floor estimate.
//!
//! Usage:
//!   cog-coherence --once
//!   cog-coherence --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Signal quality metrics for a single channel
struct ChannelQuality {
    snr_db: f64,
    stationarity: f64,
    continuity: f64,
    dynamic_range: f64,
}

/// Estimate noise floor from signal (median of absolute deviations)
fn noise_floor(signal: &[f64]) -> f64 {
    if signal.is_empty() { return 0.0; }
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let mut deviations: Vec<f64> = signal.iter().map(|v| (v - mean).abs()).collect();
    deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // MAD (Median Absolute Deviation) * 1.4826 ≈ sigma for Gaussian
    deviations[deviations.len() / 2] * 1.4826
}

/// Estimate SNR in dB
fn signal_to_noise(signal: &[f64]) -> f64 {
    let noise = noise_floor(signal);
    if noise < 1e-10 { return 60.0; } // Very clean signal
    let power = signal.iter().map(|v| v * v).sum::<f64>() / signal.len().max(1) as f64;
    let signal_power = (power - noise * noise).max(1e-10);
    10.0 * (signal_power / (noise * noise)).log10()
}

/// Test stationarity: compare mean/variance of first half vs second half
fn stationarity_score(signal: &[f64]) -> f64 {
    if signal.len() < 4 { return 1.0; }
    let mid = signal.len() / 2;
    let (first, second) = signal.split_at(mid);

    let mean1 = first.iter().sum::<f64>() / first.len() as f64;
    let mean2 = second.iter().sum::<f64>() / second.len() as f64;
    let var1 = first.iter().map(|v| (v - mean1).powi(2)).sum::<f64>() / first.len() as f64;
    let var2 = second.iter().map(|v| (v - mean2).powi(2)).sum::<f64>() / second.len() as f64;

    let mean_diff = (mean1 - mean2).abs() / (mean1.abs() + mean2.abs() + 1e-10).max(1e-10);
    let var_ratio = if var1 > 1e-10 && var2 > 1e-10 {
        (var1 / var2).min(var2 / var1)
    } else {
        1.0
    };

    // Score: 1.0 = perfectly stationary
    ((1.0 - mean_diff) * var_ratio).max(0.0).min(1.0)
}

/// Check signal continuity (no large gaps/jumps)
fn continuity_score(signal: &[f64]) -> f64 {
    if signal.len() < 2 { return 1.0; }
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let std_dev = (signal.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / signal.len() as f64).sqrt();
    if std_dev < 1e-10 { return 1.0; }

    let jumps: usize = signal.windows(2)
        .filter(|w| (w[1] - w[0]).abs() > 3.0 * std_dev)
        .count();

    (1.0 - jumps as f64 / signal.len() as f64).max(0.0)
}

fn analyze_channel(signal: &[f64]) -> ChannelQuality {
    let max = signal.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = signal.iter().cloned().fold(f64::INFINITY, f64::min);

    ChannelQuality {
        snr_db: signal_to_noise(signal),
        stationarity: stationarity_score(signal),
        continuity: continuity_score(signal),
        dynamic_range: if max > min { 20.0 * (max / min.abs().max(1e-10)).abs().log10() } else { 0.0 },
    }
}

#[derive(serde::Serialize)]
struct CoherenceResult {
    channel_count: usize,
    overall_quality: f64,
    overall_status: String,
    noise_floor: f64,
    avg_snr_db: f64,
    avg_stationarity: f64,
    avg_continuity: f64,
    per_channel: Vec<ChannelReport>,
    anomalies: Vec<String>,
    vector: [f64; DIM],
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct ChannelReport {
    channel: String,
    quality_score: f64,
    snr_db: f64,
    status: String,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
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

fn run_once() -> Result<CoherenceResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    if channels.is_empty() { return Err("no channels".into()); }

    let mut reports = Vec::new();
    let mut total_snr = 0.0;
    let mut total_stat = 0.0;
    let mut total_cont = 0.0;
    let mut total_noise = 0.0;
    let mut anomalies = Vec::new();

    for (name, vals) in &channels {
        let q = analyze_channel(vals);
        let quality = (q.snr_db.min(40.0) / 40.0 * 0.4
            + q.stationarity * 0.3
            + q.continuity * 0.3)
            .max(0.0).min(1.0);

        let status = if quality > 0.8 { "good" }
            else if quality > 0.5 { "fair" }
            else { "poor" };

        if quality < 0.3 {
            anomalies.push(format!("DEGRADED_CHANNEL: {name} quality={quality:.2}"));
        }

        total_snr += q.snr_db;
        total_stat += q.stationarity;
        total_cont += q.continuity;
        total_noise += noise_floor(vals);

        reports.push(ChannelReport {
            channel: name.clone(),
            quality_score: quality,
            snr_db: q.snr_db,
            status: status.into(),
        });
    }

    let n = channels.len() as f64;
    let avg_snr = total_snr / n;
    let avg_stat = total_stat / n;
    let avg_cont = total_cont / n;
    let avg_noise = total_noise / n;
    let overall = (avg_snr.min(40.0) / 40.0 * 0.4 + avg_stat * 0.3 + avg_cont * 0.3).max(0.0).min(1.0);

    let status = if overall > 0.8 { "healthy" }
        else if overall > 0.5 { "degraded" }
        else { "poor" };

    let vector = [
        overall,
        avg_snr / 40.0,
        avg_stat,
        avg_cont,
        avg_noise,
        channels.len() as f64 / 10.0,
        if anomalies.is_empty() { 0.0 } else { 1.0 },
        reports.iter().filter(|r| r.status == "poor").count() as f64 / n,
    ];

    let _ = store_vector(&vector);

    Ok(CoherenceResult {
        channel_count: channels.len(),
        overall_quality: overall,
        overall_status: status.into(),
        noise_floor: avg_noise,
        avg_snr_db: avg_snr,
        avg_stationarity: avg_stat,
        avg_continuity: avg_cont,
        per_channel: reports,
        anomalies,
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

    eprintln!("[cog-coherence] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-coherence] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-coherence] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
