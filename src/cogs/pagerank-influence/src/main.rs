//! Cognitum Cog: PageRank Influence
//!
//! Graph-based influence detection. Build adjacency matrix from signal
//! correlations between channels. Run PageRank iterations to find most
//! influential channels/zones.
//!
//! Usage:
//!   cog-pagerank-influence --once
//!   cog-pagerank-influence --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

/// Compute Pearson correlation between two series
fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let mean_a = a[..n].iter().sum::<f64>() / n as f64;
    let mean_b = b[..n].iter().sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }
    let denom = (var_a * var_b).sqrt();
    if denom < 1e-10 {
        return 0.0;
    }
    cov / denom
}

/// Run PageRank on an adjacency matrix (NxN, row-major)
/// Returns rank scores for each node
fn pagerank(adj: &[f64], n: usize, damping: f64, iterations: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }

    // Build transition matrix from adjacency
    let mut trans = vec![0.0; n * n];
    for j in 0..n {
        let col_sum: f64 = (0..n).map(|i| adj[i * n + j].max(0.0)).sum();
        if col_sum > 1e-10 {
            for i in 0..n {
                trans[i * n + j] = adj[i * n + j].max(0.0) / col_sum;
            }
        } else {
            // Dangling node: distribute evenly
            for i in 0..n {
                trans[i * n + j] = 1.0 / n as f64;
            }
        }
    }

    let mut rank = vec![1.0 / n as f64; n];
    let teleport = (1.0 - damping) / n as f64;

    for _ in 0..iterations {
        let mut new_rank = vec![teleport; n];
        for i in 0..n {
            for j in 0..n {
                new_rank[i] += damping * trans[i * n + j] * rank[j];
            }
        }
        // Normalize
        let sum: f64 = new_rank.iter().sum();
        if sum > 1e-10 {
            for r in &mut new_rank {
                *r /= sum;
            }
        }
        rank = new_rank;
    }
    rank
}

#[derive(serde::Serialize)]
struct InfluenceResult {
    channel_count: usize,
    pagerank_scores: Vec<f64>,
    most_influential: usize,
    most_influential_score: f64,
    least_influential: usize,
    influence_spread: f64,
    anomalies: Vec<String>,
    vector: [f64; 8],
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

fn store_vector(v: &[f64; 8]) -> Result<(), String> {
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

fn run_once() -> Result<InfluenceResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    // Group samples by channel
    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    let ch_names: Vec<String> = channels.keys().cloned().collect();
    let n = ch_names.len();
    if n < 2 {
        return Err("need at least 2 channels for graph analysis".into());
    }

    let ch_data: Vec<&Vec<f64>> = ch_names.iter().map(|k| &channels[k]).collect();

    // Build correlation adjacency matrix
    let mut adj = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            if i != j {
                let corr = correlation(ch_data[i], ch_data[j]).abs();
                adj[i * n + j] = corr;
            }
        }
    }

    // Run PageRank
    let ranks = pagerank(&adj, n, 0.85, 30);

    let (most_idx, &most_score) = ranks.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));

    let (least_idx, _) = ranks.iter().enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));

    let spread = ranks.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        - ranks.iter().cloned().fold(f64::INFINITY, f64::min);

    let mut anomalies = Vec::new();
    if most_score > 0.5 {
        anomalies.push(format!("DOMINANT_CHANNEL: ch{most_idx} score={most_score:.3}"));
    }
    if spread > 0.3 {
        anomalies.push(format!("HIGH_INFLUENCE_SPREAD: {spread:.3}"));
    }

    // Build output vector (pad/truncate to 8)
    let mut vector = [0.0; 8];
    for (i, &r) in ranks.iter().enumerate().take(6) {
        vector[i] = r;
    }
    vector[6] = spread;
    vector[7] = n as f64 / 10.0;

    let _ = store_vector(&vector);

    Ok(InfluenceResult {
        channel_count: n,
        pagerank_scores: ranks,
        most_influential: most_idx,
        most_influential_score: most_score,
        least_influential: least_idx,
        influence_spread: spread,
        anomalies,
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-pagerank-influence] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-pagerank-influence] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-pagerank-influence] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
