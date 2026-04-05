//! Cognitum Cog: DTW Gesture Learn
//!
//! Dynamic Time Warping for custom gesture recognition.
//! Record gesture templates (store as vectors), match new signals
//! against templates using DTW distance. Report best match + confidence.
//!
//! Usage:
//!   cog-dtw-gesture-learn --once
//!   cog-dtw-gesture-learn --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

/// DTW distance between two sequences
fn dtw_distance(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    let m = b.len();
    if n == 0 || m == 0 {
        return f64::MAX;
    }
    // Cost matrix (flattened)
    let mut cost = vec![f64::MAX; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    cost[idx(0, 0)] = 0.0;

    for i in 1..=n {
        for j in 1..=m {
            let d = (a[i - 1] - b[j - 1]).abs();
            let prev = cost[idx(i - 1, j)]
                .min(cost[idx(i, j - 1)])
                .min(cost[idx(i - 1, j - 1)]);
            cost[idx(i, j)] = d + prev;
        }
    }
    cost[idx(n, m)]
}

/// Normalize DTW distance to [0,1] confidence
fn dtw_confidence(dist: f64, seq_len: usize) -> f64 {
    if seq_len == 0 {
        return 0.0;
    }
    let norm = dist / seq_len as f64;
    (1.0 - norm.min(2.0) / 2.0).max(0.0)
}

/// Encode a signal window into an 8-dim gesture feature vector
fn gesture_features(signal: &[f64]) -> [f64; 8] {
    let n = signal.len().max(1) as f64;
    let mean = signal.iter().sum::<f64>() / n;
    let variance = signal.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    // Zero crossings
    let mut zc = 0usize;
    for i in 1..signal.len() {
        if (signal[i - 1] >= 0.0) != (signal[i] >= 0.0) {
            zc += 1;
        }
    }

    // Peak count
    let mut peaks = 0usize;
    for i in 1..signal.len().saturating_sub(1) {
        if signal[i] > signal[i - 1] && signal[i] > signal[i + 1] {
            peaks += 1;
        }
    }

    // Energy
    let energy = signal.iter().map(|v| v * v).sum::<f64>() / n;

    // Max absolute derivative
    let max_deriv = signal
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f64, f64::max);

    // Slope (linear regression)
    let slope = if signal.len() > 1 {
        let x_mean = (signal.len() - 1) as f64 / 2.0;
        let num: f64 = signal
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64 - x_mean) * (v - mean))
            .sum();
        let den: f64 = (0..signal.len())
            .map(|i| (i as f64 - x_mean).powi(2))
            .sum();
        if den > 1e-10 { num / den } else { 0.0 }
    } else {
        0.0
    };

    // Range
    let range = signal.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        - signal.iter().cloned().fold(f64::INFINITY, f64::min);

    [
        mean,
        std_dev,
        zc as f64 / n,
        peaks as f64 / n,
        energy,
        max_deriv,
        slope,
        range,
    ]
}

#[derive(serde::Serialize)]
struct GestureResult {
    best_match_distance: f64,
    confidence: f64,
    template_count: usize,
    signal_length: usize,
    features: [f64; 8],
    stored: bool,
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

fn query_templates(features: &[f64; 8]) -> Result<Vec<Vec<f64>>, String> {
    let payload = serde_json::json!({
        "vector": features,
        "k": 5,
        "metric": "cosine"
    });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/query HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&resp);
    let json_start = text.find('{').or_else(|| text.find('[')).unwrap_or(0);
    let parsed: serde_json::Value =
        serde_json::from_str(&text[json_start..]).unwrap_or(serde_json::json!({"results":[]}));
    let results = parsed
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let mut vecs = Vec::new();
    for r in results {
        if let Some(arr) = r.get("vector").and_then(|v| v.as_array()) {
            let v: Vec<f64> = arr.iter().filter_map(|x| x.as_f64()).collect();
            if v.len() == 8 {
                vecs.push(v);
            }
        }
    }
    Ok(vecs)
}

fn store_vector(features: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({
        "vectors": [[0, features]],
        "dedup": true
    });
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
    let samples = sensors
        .get("samples")
        .and_then(|s| s.as_array())
        .ok_or("no samples")?;
    let signal: Vec<f64> = samples
        .iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();
    if signal.is_empty() {
        return Err("no sensor readings".into());
    }

    let features = gesture_features(&signal);

    // Query store for existing templates
    let templates = query_templates(&features).unwrap_or_default();

    let (best_dist, confidence) = if templates.is_empty() {
        (f64::MAX, 0.0)
    } else {
        let mut best = f64::MAX;
        for t in &templates {
            let d = dtw_distance(&features, t);
            if d < best {
                best = d;
            }
        }
        (best, dtw_confidence(best, 8))
    };

    // Store as new template
    let stored = store_vector(&features).is_ok();

    Ok(GestureResult {
        best_match_distance: if best_dist == f64::MAX { -1.0 } else { best_dist },
        confidence,
        template_count: templates.len(),
        signal_length: signal.len(),
        features,
        stored,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args
        .iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-dtw-gesture-learn] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if r.confidence < 0.3 && r.template_count > 0 {
                    eprintln!("[cog-dtw-gesture-learn] ALERT: unknown gesture (confidence={:.2})", r.confidence);
                }
            }
            Err(e) => eprintln!("[cog-dtw-gesture-learn] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
