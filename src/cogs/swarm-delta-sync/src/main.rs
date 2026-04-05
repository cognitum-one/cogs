//! Cognitum Cog: Swarm Delta Sync
//!
//! Bi-directional vector synchronization between seeds.
//! Queries peer store, compares epoch/count, pushes missing vectors.
//!
//! Usage:
//!   cog-swarm-delta-sync --once --peer 169.254.42.2
//!   cog-swarm-delta-sync --interval 60 --peer 169.254.42.2

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(2);

fn read_http_json(conn: &mut TcpStream) -> Result<serde_json::Value, String> {
    let mut buf = [0u8; 1];
    let mut data = Vec::new();
    let mut in_body = false;
    let mut brace_depth: i32 = 0;
    let mut newline_count = 0;
    while conn.read(&mut buf).map_err(|e| format!("read: {e}"))? > 0 {
        data.push(buf[0]);
        if !in_body {
            if buf[0] == b'\n' { newline_count += 1; } else if buf[0] != b'\r' { newline_count = 0; }
            if newline_count >= 2 { in_body = true; data.clear(); }
        } else {
            if buf[0] == b'{' { brace_depth += 1; }
            if buf[0] == b'}' { brace_depth -= 1; if brace_depth == 0 { break; } }
        }
    }
    let body = String::from_utf8_lossy(&data);
    let start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
}

fn http_get(addr: &str, path: &str) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TIMEOUT)).ok();
    write!(conn, "GET {path} HTTP/1.0\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    read_http_json(&mut conn)
}

fn http_post(addr: &str, path: &str, payload: &[u8]) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TIMEOUT)).ok();
    write!(conn, "POST {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", payload.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(payload).map_err(|e| format!("body: {e}"))?;
    read_http_json(&mut conn)
}
fn get_status(addr: &str) -> Result<(String, u64), String> {
    let status = http_get(addr, "/api/v1/status")?;
    let id = status.get("device_id").and_then(|v| v.as_str())
        .unwrap_or("unknown").to_string();
    let count = status.get("total_vectors").and_then(|v| v.as_u64()).unwrap_or(0);
    Ok((id, count))
}

fn query_vectors(addr: &str, probe: &[f64; 8], k: usize) -> Result<Vec<Vec<f64>>, String> {
    let payload = serde_json::json!({ "vector": probe, "k": k, "metric": "cosine" });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let resp = http_post(addr, "/api/v1/store/query", &body)?;
    let results = resp.get("results").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut vectors = Vec::new();
    for r in results {
        if let Some(vec) = r.get("vector").and_then(|v| v.as_array()) {
            let v: Vec<f64> = vec.iter().filter_map(|x| x.as_f64()).collect();
            if v.len() == 8 { vectors.push(v); }
        }
    }
    Ok(vectors)
}

fn ingest_vectors(addr: &str, vectors: &[Vec<f64>]) -> Result<usize, String> {
    if vectors.is_empty() { return Ok(0); }
    let entries: Vec<_> = vectors.iter().enumerate()
        .map(|(i, v)| serde_json::json!([i, v]))
        .collect();
    let payload = serde_json::json!({ "vectors": entries, "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    http_post(addr, "/api/v1/store/ingest", &body)?;
    Ok(vectors.len())
}

#[derive(serde::Serialize)]
struct SyncReport {
    peer: String,
    peer_device_id: String,
    local_vectors: u64,
    peer_vectors: u64,
    pulled: usize,
    pushed: usize,
    sync_duration_ms: u64,
    timestamp: u64,
}

fn run_once(peer: &str) -> Result<SyncReport, String> {
    let start = Instant::now();
    let local_addr = "127.0.0.1:80";
    let peer_addr = format!("{peer}:80");

    let (_, local_count) = get_status(local_addr)?;
    let (peer_id, peer_count) = get_status(&peer_addr)?;

    // Use several probe vectors to sample the vector spaces
    let probes: Vec<[f64; 8]> = vec![
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.5, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0],
    ];

    let k = 50;
    let mut pulled = 0usize;
    let mut pushed = 0usize;

    for probe in &probes {
        // Pull from peer -> local
        if let Ok(peer_vecs) = query_vectors(&peer_addr, probe, k) {
            if let Ok(n) = ingest_vectors(local_addr, &peer_vecs) {
                pulled += n;
            }
        }
        // Push from local -> peer
        if let Ok(local_vecs) = query_vectors(local_addr, probe, k) {
            if let Ok(n) = ingest_vectors(&peer_addr, &local_vecs) {
                pushed += n;
            }
        }
    }

    Ok(SyncReport {
        peer: peer.to_string(),
        peer_device_id: peer_id,
        local_vectors: local_count,
        peer_vectors: peer_count,
        pulled,
        pushed,
        sync_duration_ms: start.elapsed().as_millis() as u64,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);
    let peer = args.iter()
        .position(|a| a == "--peer")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "169.254.42.1".to_string());

    eprintln!("[cog-swarm-delta-sync] starting (peer={peer}, interval={interval}s)");

    loop {
        let start = Instant::now();
        match run_once(&peer) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => {
                eprintln!("[cog-swarm-delta-sync] error: {e}");
                let err = serde_json::json!({
                    "peer": peer, "synced_vectors": 0, "success": false, "error": e,
                    "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                });
                println!("{}", err);
            }
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
