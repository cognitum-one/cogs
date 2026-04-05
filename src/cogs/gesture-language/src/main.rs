//! Cognitum Cog: Gesture Language
//!
//! Recognizes predefined gesture patterns using Dynamic Time Warping (DTW).
//! Stores gesture templates and matches incoming signal windows against them.
//!
//! Usage:
//!   cog-gesture-language --once
//!   cog-gesture-language --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

/// DTW distance between two sequences
fn dtw_distance(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    let m = b.len();
    if n == 0 || m == 0 { return f64::MAX; }

    let mut cost = vec![vec![f64::MAX; m + 1]; n + 1];
    cost[0][0] = 0.0;

    for i in 1..=n {
        for j in 1..=m {
            let d = (a[i - 1] - b[j - 1]).powi(2);
            cost[i][j] = d + cost[i - 1][j].min(cost[i][j - 1]).min(cost[i - 1][j - 1]);
        }
    }
    cost[n][m].sqrt()
}

/// Built-in gesture templates (normalized signal patterns)
struct GestureTemplate {
    name: &'static str,
    pattern: &'static [f64],
}

const TEMPLATES: &[GestureTemplate] = &[
    GestureTemplate { name: "wave", pattern: &[0.0, 0.8, 0.0, -0.8, 0.0, 0.8, 0.0, -0.8] },
    GestureTemplate { name: "push", pattern: &[0.0, 0.2, 0.5, 0.9, 1.0, 0.7, 0.3, 0.0] },
    GestureTemplate { name: "pull", pattern: &[1.0, 0.7, 0.3, 0.0, -0.2, -0.3, -0.2, 0.0] },
    GestureTemplate { name: "circle", pattern: &[0.0, 0.7, 1.0, 0.7, 0.0, -0.7, -1.0, -0.7] },
    GestureTemplate { name: "tap", pattern: &[0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0] },
    GestureTemplate { name: "swipe-left", pattern: &[0.0, -0.3, -0.6, -0.9, -1.0, -0.8, -0.4, 0.0] },
    GestureTemplate { name: "swipe-right", pattern: &[0.0, 0.3, 0.6, 0.9, 1.0, 0.8, 0.4, 0.0] },
    GestureTemplate { name: "shake", pattern: &[0.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0] },
];

#[derive(serde::Serialize)]
struct GestureReport {
    gesture: String,
    distance: f64,
    confidence: f64,
    all_matches: Vec<GestureMatch>,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct GestureMatch {
    name: String,
    distance: f64,
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

/// Normalize a signal to [-1, 1] range
fn normalize(values: &[f64]) -> Vec<f64> {
    if values.is_empty() { return vec![]; }
    let min = values.iter().cloned().fold(f64::MAX, f64::min);
    let max = values.iter().cloned().fold(f64::MIN, f64::max);
    let range = max - min;
    if range < 1e-10 { return vec![0.0; values.len()]; }
    values.iter().map(|v| (v - min) / range * 2.0 - 1.0).collect()
}

fn run_once() -> Result<GestureReport, String> {
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

    let normalized = normalize(&values);

    // DTW match against all templates
    let mut matches: Vec<GestureMatch> = TEMPLATES.iter().map(|t| {
        let dist = dtw_distance(&normalized, t.pattern);
        GestureMatch { name: t.name.into(), distance: dist }
    }).collect();

    matches.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

    let best = &matches[0];
    let worst_dist = matches.last().map(|m| m.distance).unwrap_or(1.0);
    let confidence = if worst_dist > 1e-10 { 1.0 - (best.distance / worst_dist) } else { 0.0 };

    let report = GestureReport {
        gesture: best.name.clone(),
        distance: best.distance,
        confidence,
        all_matches: matches,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    // Store: encode gesture index + distance + confidence
    let gesture_idx = TEMPLATES.iter().position(|t| t.name == report.gesture).unwrap_or(0) as f64;
    let vec8 = [
        gesture_idx / TEMPLATES.len() as f64,
        report.distance / 10.0,
        report.confidence,
        normalized.len() as f64 / 32.0,
        0.0, 0.0, 0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-gesture-language] store error: {e}");
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

    eprintln!("[cog-gesture-language] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.confidence > 0.6 {
                    eprintln!("[cog-gesture-language] ALERT: detected '{}' (conf={:.2}, dist={:.2})",
                        report.gesture, report.confidence, report.distance);
                }
            }
            Err(e) => eprintln!("[cog-gesture-language] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
