//! Cognitum Cog: Gesture
//!
//! Core gesture recognition building block. Template-based DTW matching
//! with configurable gesture library. Other cogs can use this as a dependency.
//!
//! Usage:
//!   cog-gesture --once
//!   cog-gesture --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// DTW distance with Sakoe-Chiba band constraint for efficiency
fn dtw_banded(a: &[f64], b: &[f64], band: usize) -> f64 {
    let n = a.len();
    let m = b.len();
    if n == 0 || m == 0 { return f64::MAX; }

    let mut cost = vec![f64::MAX; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    cost[idx(0, 0)] = 0.0;

    for i in 1..=n {
        let j_start = if i > band { i - band } else { 1 };
        let j_end = (i + band).min(m);
        for j in j_start..=j_end {
            let d = (a[i - 1] - b[j - 1]).powi(2);
            let prev = cost[idx(i - 1, j)]
                .min(cost[idx(i, j - 1)])
                .min(cost[idx(i - 1, j - 1)]);
            cost[idx(i, j)] = d + prev;
        }
    }
    cost[idx(n, m)].sqrt()
}

/// Predefined gesture templates
struct GestureTemplate {
    name: &'static str,
    /// Characteristic signal shape (normalized)
    pattern: &'static [f64],
}

const TEMPLATES: &[GestureTemplate] = &[
    GestureTemplate {
        name: "wave",
        pattern: &[0.0, 0.5, 1.0, 0.5, 0.0, -0.5, -1.0, -0.5, 0.0, 0.5, 1.0, 0.5, 0.0],
    },
    GestureTemplate {
        name: "tap",
        pattern: &[0.0, 0.0, 0.8, 1.0, 0.3, 0.0, 0.0, 0.0],
    },
    GestureTemplate {
        name: "swipe_right",
        pattern: &[0.0, 0.2, 0.5, 0.8, 1.0, 0.9, 0.7, 0.3, 0.0],
    },
    GestureTemplate {
        name: "swipe_left",
        pattern: &[0.0, -0.2, -0.5, -0.8, -1.0, -0.9, -0.7, -0.3, 0.0],
    },
    GestureTemplate {
        name: "circle",
        pattern: &[0.0, 0.7, 1.0, 0.7, 0.0, -0.7, -1.0, -0.7, 0.0],
    },
    GestureTemplate {
        name: "push",
        pattern: &[0.0, 0.3, 0.7, 1.0, 1.0, 0.8, 0.4, 0.0],
    },
    GestureTemplate {
        name: "pull",
        pattern: &[1.0, 0.8, 0.4, 0.0, 0.0, 0.3, 0.7, 1.0],
    },
    GestureTemplate {
        name: "still",
        pattern: &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    },
];

/// Normalize signal to [-1, 1] range
fn normalize_signal(signal: &[f64]) -> Vec<f64> {
    if signal.is_empty() { return Vec::new(); }
    let max = signal.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = signal.iter().cloned().fold(f64::INFINITY, f64::min);
    let range = max - min;
    if range < 1e-10 {
        return vec![0.0; signal.len()];
    }
    signal.iter().map(|v| 2.0 * (v - min) / range - 1.0).collect()
}

/// Match signal against all templates, return sorted matches
fn match_templates(signal: &[f64]) -> Vec<(String, f64, f64)> {
    let norm = normalize_signal(signal);
    let band = norm.len().max(TEMPLATES.iter().map(|t| t.pattern.len()).max().unwrap_or(8)) / 3;

    let mut matches: Vec<(String, f64, f64)> = TEMPLATES.iter().map(|tmpl| {
        let dist = dtw_banded(&norm, tmpl.pattern, band.max(2));
        let max_dist = (norm.len() + tmpl.pattern.len()) as f64;
        let confidence = (1.0 - dist / max_dist).max(0.0).min(1.0);
        (tmpl.name.to_string(), dist, confidence)
    }).collect();

    matches.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    matches
}

#[derive(serde::Serialize)]
struct GestureResult {
    detected_gesture: String,
    confidence: f64,
    distance: f64,
    all_matches: Vec<GestureMatch>,
    signal_length: usize,
    anomalies: Vec<String>,
    vector: [f64; DIM],
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct GestureMatch {
    gesture: String,
    distance: f64,
    confidence: f64,
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

fn run_once() -> Result<GestureResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let signal: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if signal.is_empty() { return Err("no sensor readings".into()); }

    let matches = match_templates(&signal);
    let top = matches.first().cloned().unwrap_or(("unknown".into(), f64::MAX, 0.0));

    let all_matches: Vec<GestureMatch> = matches.iter().take(5).map(|(g, d, c)| {
        GestureMatch { gesture: g.clone(), distance: *d, confidence: *c }
    }).collect();

    let mut anomalies = Vec::new();
    if top.2 < 0.3 {
        anomalies.push("LOW_CONFIDENCE: gesture unclear".into());
    }
    // Check if top two are very close (ambiguous)
    if matches.len() >= 2 && (matches[1].2 - matches[0].2).abs() < 0.05 {
        anomalies.push(format!("AMBIGUOUS: {} vs {} nearly equal", matches[0].0, matches[1].0));
    }

    let gesture_idx = TEMPLATES.iter().position(|t| t.name == top.0).unwrap_or(0);
    let vector = [
        gesture_idx as f64 / TEMPLATES.len() as f64,
        top.2, // confidence
        top.1.min(10.0) / 10.0, // distance normalized
        signal.len() as f64 / 100.0,
        matches.get(1).map(|m| m.2).unwrap_or(0.0), // 2nd best confidence
        if anomalies.is_empty() { 0.0 } else { 1.0 },
        0.0,
        0.0,
    ];

    let _ = store_vector(&vector);

    Ok(GestureResult {
        detected_gesture: top.0,
        confidence: top.2,
        distance: top.1,
        all_matches,
        signal_length: signal.len(),
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
        .unwrap_or(5);

    eprintln!("[cog-gesture] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-gesture] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-gesture] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
