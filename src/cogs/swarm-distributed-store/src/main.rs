//! Cognitum Cog: Swarm Distributed Store
//!
//! Hash-based vector partitioning across seeds. Fan-out queries to all
//! peers in parallel, merge results by distance for unified search.
//!
//! Usage:
//!   cog-swarm-distributed-store --once --peers 169.254.42.2,169.254.42.3
//!   cog-swarm-distributed-store --interval 10 --peers 169.254.42.2

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(3);

fn http_post(addr: &str, path: &str, payload: &[u8]) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TIMEOUT)).ok();
    write!(conn, "POST {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", payload.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(payload).map_err(|e| format!("body: {e}"))?;
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
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn http_get(addr: &str, path: &str) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TIMEOUT)).ok();
    write!(conn, "GET {path} HTTP/1.0\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
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
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

/// Simple hash to assign vector to a partition
fn hash_vector(vec: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &v in vec {
        let bits = v.to_bits();
        h ^= bits;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn cosine_distance(a: &[f64], b: &[f64]) -> f64 {
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for i in 0..a.len().min(b.len()) {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1e-12 { 1.0 } else { 1.0 - dot / denom }
}

#[derive(serde::Serialize, Clone)]
struct SearchResult {
    vector: Vec<f64>,
    distance: f64,
    source: String,
}

#[derive(serde::Serialize)]
struct DistributedReport {
    mode: String,
    peers_queried: usize,
    peers_responded: usize,
    results: Vec<SearchResult>,
    total_results: usize,
    query_duration_ms: u64,
    timestamp: u64,
}

fn fan_out_query(peers: &[String], query_vec: &[f64; 8], k: usize) -> DistributedReport {
    let start = Instant::now();
    let mut all_results: Vec<SearchResult> = Vec::new();
    let mut responded = 0usize;
    let all_addrs: Vec<String> = std::iter::once("127.0.0.1:80".to_string())
        .chain(peers.iter().map(|p| format!("{p}:80")))
        .collect();

    for addr in &all_addrs {
        let payload = serde_json::json!({ "vector": query_vec, "k": k, "metric": "cosine" });
        let body = match serde_json::to_vec(&payload) { Ok(b) => b, Err(_) => continue };
        match http_post(addr, "/api/v1/store/query", &body) {
            Ok(resp) => {
                responded += 1;
                if let Some(results) = resp.get("results").and_then(|v| v.as_array()) {
                    for r in results {
                        if let Some(vec) = r.get("vector").and_then(|v| v.as_array()) {
                            let v: Vec<f64> = vec.iter().filter_map(|x| x.as_f64()).collect();
                            if v.len() == 8 {
                                let dist = cosine_distance(&v, query_vec);
                                all_results.push(SearchResult {
                                    vector: v,
                                    distance: dist,
                                    source: addr.clone(),
                                });
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }

    // Sort by distance, take top k
    all_results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(k);
    let total = all_results.len();

    DistributedReport {
        mode: "fan-out-query".to_string(),
        peers_queried: all_addrs.len(),
        peers_responded: responded,
        results: all_results,
        total_results: total,
        query_duration_ms: start.elapsed().as_millis() as u64,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    }
}

fn distribute_ingest(peers: &[String], vectors: &[Vec<f64>]) -> serde_json::Value {
    let n_partitions = peers.len() + 1; // include local
    let mut partitions: Vec<Vec<Vec<f64>>> = vec![Vec::new(); n_partitions];

    for v in vectors {
        let h = hash_vector(v);
        let idx = (h as usize) % n_partitions;
        partitions[idx].push(v.clone());
    }

    let all_addrs: Vec<String> = std::iter::once("127.0.0.1:80".to_string())
        .chain(peers.iter().map(|p| format!("{p}:80")))
        .collect();

    let mut results = serde_json::Map::new();
    for (i, (addr, vecs)) in all_addrs.iter().zip(partitions.iter()).enumerate() {
        if vecs.is_empty() { continue; }
        let entries: Vec<_> = vecs.iter().enumerate()
            .map(|(j, v)| serde_json::json!([j, v]))
            .collect();
        let payload = serde_json::json!({ "vectors": entries, "dedup": true });
        let body = match serde_json::to_vec(&payload) { Ok(b) => b, Err(_) => continue };
        let status = match http_post(addr, "/api/v1/store/ingest", &body) {
            Ok(_) => "ok",
            Err(_) => "error",
        };
        results.insert(format!("partition_{i}"), serde_json::json!({
            "addr": addr, "vectors": vecs.len(), "status": status
        }));
    }
    serde_json::Value::Object(results)
}

fn run_once(peers: &[String]) -> Result<DistributedReport, String> {
    // Read local sensors, build a query vector for demonstration
    let sensors = {
        let mut conn = TcpStream::connect("127.0.0.1:80")
            .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
        write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .map_err(|e| format!("write: {e}"))?;
        let mut buf = Vec::new();
        conn.read_to_end(&mut buf).map_err(|e| format!("read: {e}"))?;
        let body = String::from_utf8_lossy(&buf);
        let json_start = body.find('{').ok_or("no JSON")?;
        let v: serde_json::Value = serde_json::from_str(&body[json_start..])
            .map_err(|e| format!("parse: {e}"))?;
        v
    };

    let samples = sensors.get("samples").and_then(|c| c.as_array()).ok_or("no samples")?;
    let mut query_vec = [0.0f64; 8];
    for (i, ch) in samples.iter().take(8).enumerate() {
        query_vec[i] = ch.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
    }

    // Ingest locally-generated vector to a partition
    let _ = distribute_ingest(peers, &[query_vec.to_vec()]);

    // Fan-out query across all peers
    Ok(fan_out_query(peers, &query_vec, 5))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);
    let peers: Vec<String> = args.iter()
        .position(|a| a == "--peers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    eprintln!("[cog-swarm-distributed-store] starting (peers={}, interval={interval}s)", peers.len());

    loop {
        let start = Instant::now();
        match run_once(&peers) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-swarm-distributed-store] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
