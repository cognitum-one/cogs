//! Cognitum Cog: Adversarial
//!
//! Detect tampered/spoofed signals. Statistical tests for artificial
//! regularity (too-perfect signals), impossible value ranges, and
//! temporal inconsistencies.
//!
//! Usage:
//!   cog-adversarial --once
//!   cog-adversarial --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Test for artificial regularity: truly random signals have specific entropy
fn regularity_score(signal: &[f64]) -> f64 {
    if signal.len() < 4 { return 0.0; }

    // Compute first-order differences
    let diffs: Vec<f64> = signal.windows(2).map(|w| w[1] - w[0]).collect();
    let mean_diff = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let var_diff = diffs.iter().map(|d| (d - mean_diff).powi(2)).sum::<f64>() / diffs.len() as f64;

    // Check for suspiciously constant differences (linear signal = spoofed)
    let cv = if mean_diff.abs() > 1e-10 {
        var_diff.sqrt() / mean_diff.abs()
    } else {
        var_diff.sqrt()
    };

    // Very low CV in differences = suspiciously regular
    if cv < 0.01 { 1.0 } // Almost certainly fake
    else if cv < 0.1 { 0.7 }
    else if cv < 0.3 { 0.3 }
    else { 0.0 } // Natural signal
}

/// Test for impossible value ranges
fn range_anomaly(signal: &[f64]) -> f64 {
    let max = signal.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = signal.iter().cloned().fold(f64::INFINITY, f64::min);

    let mut score: f64 = 0.0;

    // All identical values
    if (max - min).abs() < 1e-10 && signal.len() > 3 {
        score += 0.5;
    }

    // Perfectly bounded (exactly 0.0 to 1.0 with no overshoot)
    if (min - 0.0).abs() < 1e-6 && (max - 1.0).abs() < 1e-6 && signal.len() > 10 {
        score += 0.3;
    }

    // Values are all exact integers (suspicious for analog sensors)
    let all_integers = signal.iter().all(|v| (*v - v.round()).abs() < 1e-6);
    if all_integers && signal.len() > 5 {
        score += 0.2;
    }

    score.min(1.0)
}

/// Test for temporal inconsistencies
fn temporal_anomaly(signal: &[f64]) -> f64 {
    if signal.len() < 6 { return 0.0; }

    let mut score: f64 = 0.0;

    // Check for exact periodicity (replay attack)
    let half = signal.len() / 2;
    if half > 3 {
        let mut match_count = 0;
        for i in 0..half {
            if (signal[i] - signal[i + half]).abs() < 1e-6 {
                match_count += 1;
            }
        }
        let match_ratio = match_count as f64 / half as f64;
        if match_ratio > 0.95 {
            score += 0.8; // Very likely replayed
        } else if match_ratio > 0.8 {
            score += 0.4;
        }
    }

    // Check for monotonicity (ramp injection)
    let increasing = signal.windows(2).filter(|w| w[1] >= w[0]).count();
    let decreasing = signal.windows(2).filter(|w| w[1] <= w[0]).count();
    let n = signal.len() - 1;
    if increasing == n || decreasing == n {
        score += 0.3; // Perfectly monotonic is suspicious
    }

    score.min(1.0)
}

/// Kolmogorov-Smirnov-like test: compare distribution to Gaussian
fn distribution_anomaly(signal: &[f64]) -> f64 {
    if signal.len() < 10 { return 0.0; }

    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let var = signal.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / signal.len() as f64;
    let sd = var.sqrt();
    if sd < 1e-10 { return 0.5; }

    // Check kurtosis: natural signals have kurtosis ~3 (Gaussian)
    // Spoofed uniform signals have kurtosis ~1.8
    let kurtosis = signal.iter().map(|v| ((v - mean) / sd).powi(4)).sum::<f64>() / signal.len() as f64;

    // Too low kurtosis (uniform) or too high (extreme outlier injection)
    if kurtosis < 1.5 { 0.5 }
    else if kurtosis > 10.0 { 0.3 }
    else { 0.0 }
}

#[derive(serde::Serialize)]
struct AdversarialResult {
    overall_threat_score: f64,
    regularity_score: f64,
    range_anomaly: f64,
    temporal_anomaly: f64,
    distribution_anomaly: f64,
    verdict: String,
    threats_detected: Vec<String>,
    channel_count: usize,
    vector: [f64; DIM],
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

fn run_once() -> Result<AdversarialResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    let all_values: Vec<f64> = channels.values().flatten().cloned().collect();
    if all_values.is_empty() { return Err("no data".into()); }

    let reg = regularity_score(&all_values);
    let range = range_anomaly(&all_values);
    let temporal = temporal_anomaly(&all_values);
    let dist = distribution_anomaly(&all_values);

    let threat = (reg * 0.3 + range * 0.2 + temporal * 0.3 + dist * 0.2).min(1.0);

    let mut threats = Vec::new();
    if reg > 0.5 { threats.push("ARTIFICIAL_REGULARITY: signal too regular".into()); }
    if range > 0.5 { threats.push("IMPOSSIBLE_RANGE: suspicious value bounds".into()); }
    if temporal > 0.5 { threats.push("REPLAY_ATTACK: temporal pattern repeated".into()); }
    if dist > 0.3 { threats.push("DISTRIBUTION_ANOMALY: non-natural distribution".into()); }

    let verdict = if threat > 0.7 { "likely_spoofed" }
        else if threat > 0.4 { "suspicious" }
        else if threat > 0.2 { "minor_anomaly" }
        else { "authentic" };

    let vector = [
        threat,
        reg,
        range,
        temporal,
        dist,
        channels.len() as f64 / 10.0,
        all_values.len() as f64 / 100.0,
        if threats.is_empty() { 0.0 } else { 1.0 },
    ];

    let _ = store_vector(&vector);

    Ok(AdversarialResult {
        overall_threat_score: threat,
        regularity_score: reg,
        range_anomaly: range,
        temporal_anomaly: temporal,
        distribution_anomaly: dist,
        verdict: verdict.into(),
        threats_detected: threats,
        channel_count: channels.len(),
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

    eprintln!("[cog-adversarial] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.threats_detected.is_empty() {
                    eprintln!("[cog-adversarial] ALERT: {:?}", r.threats_detected);
                }
            }
            Err(e) => eprintln!("[cog-adversarial] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
