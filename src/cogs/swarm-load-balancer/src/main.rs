//! Cognitum Cog: Swarm Load Balancer
//!
//! Distributes queries across seeds based on load (uptime, vector count,
//! response time). Routes to least-loaded peer.
//!
//! Usage:
//!   cog-swarm-load-balancer --once --peers 169.254.42.2,169.254.42.3
//!   cog-swarm-load-balancer --interval 5

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(2);

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
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
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
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

#[derive(serde::Serialize, Clone)]
struct PeerLoad {
    address: String,
    total_vectors: u64,
    uptime_secs: u64,
    response_ms: u64,
    load_score: f64,  // lower = less loaded
    reachable: bool,
}

#[derive(serde::Serialize)]
struct BalancerReport {
    peers: Vec<PeerLoad>,
    recommended_target: String,
    query_routed: bool,
    query_result: Option<serde_json::Value>,
    timestamp: u64,
}

fn compute_load_score(vectors: u64, response_ms: u64) -> f64 {
    // Lower score = better candidate
    // Weight response time heavily (indicates current CPU load)
    let vec_factor = (vectors as f64).ln().max(0.0) / 20.0;
    let latency_factor = response_ms as f64 / 1000.0;
    vec_factor * 0.3 + latency_factor * 0.7
}

fn probe_peer(ip: &str) -> PeerLoad {
    let addr = format!("{ip}:80");
    let start = Instant::now();
    match http_get(&addr, "/api/v1/status") {
        Ok(status) => {
            let response_ms = start.elapsed().as_millis() as u64;
            let vectors = status.get("total_vectors").and_then(|v| v.as_u64()).unwrap_or(0);
            let uptime = status.get("uptime_secs").and_then(|v| v.as_u64()).unwrap_or(0);
            PeerLoad {
                address: ip.to_string(),
                total_vectors: vectors,
                uptime_secs: uptime,
                response_ms,
                load_score: compute_load_score(vectors, response_ms),
                reachable: true,
            }
        }
        Err(_) => PeerLoad {
            address: ip.to_string(),
            total_vectors: 0,
            uptime_secs: 0,
            response_ms: start.elapsed().as_millis() as u64,
            load_score: f64::MAX,
            reachable: false,
        },
    }
}

fn run_once(peers: &[String]) -> Result<BalancerReport, String> {
    let mut all_ips: Vec<String> = vec!["127.0.0.1".to_string()];
    all_ips.extend(peers.iter().cloned());

    let mut loads: Vec<PeerLoad> = all_ips.iter().map(|ip| probe_peer(ip)).collect();
    loads.sort_by(|a, b| a.load_score.partial_cmp(&b.load_score).unwrap_or(std::cmp::Ordering::Equal));

    let best = loads.iter()
        .find(|p| p.reachable)
        .map(|p| p.address.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    // Route a sample query to the least-loaded peer
    let query_vec = [0.5f64; 8];
    let payload = serde_json::json!({ "vector": query_vec, "k": 3, "metric": "cosine" });
    let body = serde_json::to_vec(&payload).unwrap_or_default();
    let best_addr = format!("{best}:80");
    let query_result = http_post(&best_addr, "/api/v1/store/query", &body).ok();

    // Store balancer state
    let store_vec = vec![
        loads.len() as f64 / 255.0,
        loads.iter().filter(|p| p.reachable).count() as f64 / loads.len().max(1) as f64,
        loads.first().map(|p| p.load_score).unwrap_or(1.0),
        loads.last().map(|p| if p.reachable { p.load_score } else { 1.0 }).unwrap_or(1.0),
        0.0, 0.0, 0.0, 0.0,
    ];
    let store_payload = serde_json::json!({ "vectors": [[0, store_vec]], "dedup": true });
    let store_body = serde_json::to_vec(&store_payload).unwrap_or_default();
    let _ = http_post("127.0.0.1:80", "/api/v1/store/ingest", &store_body);

    Ok(BalancerReport {
        peers: loads,
        recommended_target: best,
        query_routed: query_result.is_some(),
        query_result,
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
        .unwrap_or(5);
    let peers: Vec<String> = args.iter()
        .position(|a| a == "--peers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    eprintln!("[cog-swarm-load-balancer] starting (peers={}, interval={interval}s)", peers.len());

    loop {
        let start = Instant::now();
        match run_once(&peers) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-swarm-load-balancer] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
