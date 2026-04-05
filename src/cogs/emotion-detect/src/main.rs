//! Cognitum Cog: Emotion Detection
//!
//! Estimates arousal/valence from body movement patterns using the Russell
//! circumplex model. High-frequency variance maps to arousal, amplitude
//! envelope maps to energy/valence.
//!
//! Usage:
//!   cog-emotion-detect --once
//!   cog-emotion-detect --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

#[derive(serde::Serialize)]
struct EmotionReport {
    arousal: f64,
    valence: f64,
    energy: f64,
    emotion: String,
    confidence: f64,
    quadrant: String,
    timestamp: u64,
}

/// Map arousal/valence to Russell circumplex emotion label
fn classify_emotion(arousal: f64, valence: f64) -> (&'static str, &'static str) {
    match (arousal > 0.0, valence > 0.0) {
        (true, true) => ("excited", "high-arousal/positive"),
        (true, false) => ("stressed", "high-arousal/negative"),
        (false, true) => ("calm", "low-arousal/positive"),
        (false, false) => ("sad", "low-arousal/negative"),
    }
}

/// Compute high-frequency variance (arousal proxy) using successive differences
fn high_freq_variance(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let diffs: Vec<f64> = values.windows(2).map(|w| (w[1] - w[0]).powi(2)).collect();
    diffs.iter().sum::<f64>() / diffs.len() as f64
}

/// Compute amplitude envelope (energy proxy)
fn amplitude_envelope(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
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
    let json_start = body.find('{').ok_or("no JSON in response")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
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

fn run_once() -> Result<EmotionReport, String> {
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

    let hf_var = high_freq_variance(&values);
    let amp_env = amplitude_envelope(&values);

    // Normalize to [-1, 1] range using tanh
    let arousal = (hf_var / 10.0).tanh() * 2.0 - 1.0;
    let energy = (amp_env / 50.0).tanh();
    // Valence: positive when energy is moderate (not too high/low)
    let valence = (1.0 - (energy - 0.5).abs() * 2.0) * energy.signum();

    let (emotion, quadrant) = classify_emotion(arousal, valence);
    // Confidence: higher when further from origin (clearer classification)
    let confidence = ((arousal.powi(2) + valence.powi(2)).sqrt() / 1.414).min(1.0);

    let report = EmotionReport {
        arousal,
        valence,
        energy,
        emotion: emotion.into(),
        confidence,
        quadrant: quadrant.into(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        arousal, valence, energy, confidence,
        hf_var / 100.0, amp_env / 100.0, 0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-emotion-detect] store error: {e}");
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

    eprintln!("[cog-emotion-detect] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.confidence > 0.7 {
                    eprintln!("[cog-emotion-detect] ALERT: strong {} detected (conf={:.2})",
                        report.emotion, report.confidence);
                }
            }
            Err(e) => eprintln!("[cog-emotion-detect] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
