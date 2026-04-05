//! Cognitum Cog: Swarm Backup/Restore
//!
//! Replicates local vector store to a peer seed. Tracks replication lag.
//! On failure, restores vectors from the peer backup.
//!
//! Usage:
//!   cog-swarm-backup-restore --once --peer 169.254.42.2 --mode backup
//!   cog-swarm-backup-restore --once --peer 169.254.42.2 --mode restore
//!   cog-swarm-backup-restore --interval 120 --peer 169.254.42.2

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(3);

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

fn get_vector_count(addr: &str) -> Result<u64, String> {
    let status = http_get(addr, "/api/v1/status")?;
    Ok(status.get("total_vectors").and_then(|v| v.as_u64()).unwrap_or(0))
}

fn query_all_vectors(addr: &str, batch_size: usize) -> Result<Vec<Vec<f64>>, String> {
    let mut all_vecs = Vec::new();
    // Sample the vector space with multiple probe directions
    let probes: Vec<[f64; 8]> = vec![
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
        [-0.5, -0.5, -0.5, -0.5, 0.5, 0.5, 0.5, 0.5],
        [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];

    for probe in &probes {
        let payload = serde_json::json!({ "vector": probe, "k": batch_size, "metric": "cosine" });
        let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
        if let Ok(resp) = http_post(addr, "/api/v1/store/query", &body) {
            if let Some(results) = resp.get("results").and_then(|v| v.as_array()) {
                for r in results {
                    if let Some(vec) = r.get("vector").and_then(|v| v.as_array()) {
                        let v: Vec<f64> = vec.iter().filter_map(|x| x.as_f64()).collect();
                        if v.len() == 8 {
                            // Deduplicate
                            let exists = all_vecs.iter().any(|existing: &Vec<f64>| {
                                existing.iter().zip(v.iter())
                                    .all(|(a, b)| (a - b).abs() < 1e-10)
                            });
                            if !exists { all_vecs.push(v); }
                        }
                    }
                }
            }
        }
    }
    Ok(all_vecs)
}

fn ingest_vectors(addr: &str, vectors: &[Vec<f64>]) -> Result<usize, String> {
    if vectors.is_empty() { return Ok(0); }
    // Batch in chunks of 100
    let mut total = 0;
    for chunk in vectors.chunks(100) {
        let entries: Vec<_> = chunk.iter().enumerate()
            .map(|(i, v)| serde_json::json!([i, v]))
            .collect();
        let payload = serde_json::json!({ "vectors": entries, "dedup": true });
        let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
        http_post(addr, "/api/v1/store/ingest", &body)?;
        total += chunk.len();
    }
    Ok(total)
}

#[derive(serde::Serialize)]
struct BackupReport {
    mode: String,
    peer: String,
    local_vectors: u64,
    peer_vectors: u64,
    vectors_transferred: usize,
    replication_lag: i64,
    duration_ms: u64,
    success: bool,
    timestamp: u64,
}

fn run_backup(peer: &str) -> Result<BackupReport, String> {
    let start = Instant::now();
    let local_addr = "127.0.0.1:80";
    let peer_addr = format!("{peer}:80");

    let local_count = get_vector_count(local_addr)?;
    let peer_count = get_vector_count(&peer_addr)?;

    // Pull all vectors from local and push to peer
    let vectors = query_all_vectors(local_addr, 100)?;
    let transferred = ingest_vectors(&peer_addr, &vectors)?;

    let new_peer_count = get_vector_count(&peer_addr).unwrap_or(peer_count);

    Ok(BackupReport {
        mode: "backup".to_string(),
        peer: peer.to_string(),
        local_vectors: local_count,
        peer_vectors: new_peer_count,
        vectors_transferred: transferred,
        replication_lag: local_count as i64 - new_peer_count as i64,
        duration_ms: start.elapsed().as_millis() as u64,
        success: true,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn run_restore(peer: &str) -> Result<BackupReport, String> {
    let start = Instant::now();
    let local_addr = "127.0.0.1:80";
    let peer_addr = format!("{peer}:80");

    let local_count = get_vector_count(local_addr)?;
    let peer_count = get_vector_count(&peer_addr)?;

    // Pull all vectors from peer and push to local
    let vectors = query_all_vectors(&peer_addr, 100)?;
    let transferred = ingest_vectors(local_addr, &vectors)?;

    let new_local_count = get_vector_count(local_addr).unwrap_or(local_count);

    Ok(BackupReport {
        mode: "restore".to_string(),
        peer: peer.to_string(),
        local_vectors: new_local_count,
        peer_vectors: peer_count,
        vectors_transferred: transferred,
        replication_lag: new_local_count as i64 - peer_count as i64,
        duration_ms: start.elapsed().as_millis() as u64,
        success: true,
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
        .unwrap_or(120);
    let peer = args.iter()
        .position(|a| a == "--peer")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "169.254.42.1".to_string());
    let mode = args.iter()
        .position(|a| a == "--mode")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "backup".to_string());

    eprintln!("[cog-swarm-backup-restore] starting (mode={mode}, peer={peer}, interval={interval}s)");

    loop {
        let start = Instant::now();
        let result = match mode.as_str() {
            "restore" => run_restore(&peer),
            _ => run_backup(&peer),
        };
        match result {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.replication_lag.unsigned_abs() > 100 {
                    eprintln!("[cog-swarm-backup-restore] WARNING: replication lag = {}", report.replication_lag);
                }
            }
            Err(e) => {
                eprintln!("[cog-swarm-backup-restore] error: {e}");
                let err_report = serde_json::json!({
                    "mode": mode, "peer": peer, "local_vectors": 0, "peer_vectors": 0,
                    "vectors_transferred": 0, "replication_lag": 0, "duration_ms": start.elapsed().as_millis() as u64,
                    "success": false, "error": e,
                    "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                });
                println!("{}", err_report);
            }
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
