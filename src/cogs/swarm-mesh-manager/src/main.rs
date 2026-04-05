//! Cognitum Cog: Swarm Mesh Manager
//!
//! Discovers peer seeds by scanning 169.254.42.x and local subnet.
//! Queries each peer's /api/v1/status to build a mesh topology registry.
//!
//! Usage:
//!   cog-swarm-mesh-manager --once
//!   cog-swarm-mesh-manager --interval 30

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const SCAN_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PeerInfo {
    address: String,
    device_id: String,
    total_vectors: u64,
    uptime_secs: u64,
    reachable: bool,
}

#[derive(serde::Serialize)]
struct MeshReport {
    peers: Vec<PeerInfo>,
    total_peers: usize,
    healthy_peers: usize,
    total_vectors_cluster: u64,
    scan_duration_ms: u64,
    timestamp: u64,
}

fn http_get(addr: &str, path: &str, timeout: Duration) -> Result<serde_json::Value, String> {
    let conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        timeout,
    ).map_err(|e| format!("connect {addr}: {e}"))?;
    conn.set_read_timeout(Some(timeout)).ok();
    conn.set_write_timeout(Some(timeout)).ok();
    let mut conn = conn;
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

fn scan_peer(ip: &str) -> Option<PeerInfo> {
    let addr = format!("{ip}:80");
    match http_get(&addr, "/api/v1/status", SCAN_TIMEOUT) {
        Ok(status) => Some(PeerInfo {
            address: ip.to_string(),
            device_id: status.get("device_id")
                .and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            total_vectors: status.get("total_vectors")
                .and_then(|v| v.as_u64()).unwrap_or(0),
            uptime_secs: status.get("uptime_secs")
                .and_then(|v| v.as_u64()).unwrap_or(0),
            reachable: true,
        }),
        Err(_) => None,
    }
}

fn get_local_ip() -> Option<String> {
    // Try to determine local IP by connecting to a known address
    if let Ok(s) = TcpStream::connect_timeout(
        &"169.254.42.1:80".parse().unwrap(),
        Duration::from_millis(100),
    ) {
        if let Ok(local) = s.local_addr() {
            return Some(local.ip().to_string());
        }
    }
    None
}

fn store_topology(report: &MeshReport) -> Result<(), String> {
    let vector = vec![
        report.total_peers as f64 / 255.0,
        report.healthy_peers as f64 / 255.0,
        (report.total_vectors_cluster as f64).ln().max(0.0) / 20.0,
        report.scan_duration_ms as f64 / 10000.0,
        0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once() -> Result<MeshReport, String> {
    let start = Instant::now();
    let mut peers = Vec::new();

    // Always scan localhost first
    if let Some(peer) = scan_peer("127.0.0.1") {
        peers.push(peer);
    }

    // Scan common seed IPs in gadget subnet (quick scan)
    for i in 1..=10u8 {
        let ip = format!("169.254.42.{i}");
        if ip == "127.0.0.1" { continue; }
        if let Some(peer) = scan_peer(&ip) {
            peers.push(peer);
        }
    }

    // Also scan local subnet if we can determine it
    if let Some(local_ip) = get_local_ip() {
        let parts: Vec<&str> = local_ip.split('.').collect();
        if parts.len() == 4 {
            if let (Ok(a), Ok(b), Ok(c)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>(), parts[2].parse::<u8>()) {
                for i in 1..=20u8 {
                    let ip = format!("{a}.{b}.{c}.{i}");
                    if ip == local_ip { continue; }
                    if peers.iter().any(|p| p.address == ip) { continue; }
                    if let Some(peer) = scan_peer(&ip) {
                        peers.push(peer);
                    }
                }
            }
        }
    }

    let healthy = peers.iter().filter(|p| p.reachable).count();
    let total_vecs: u64 = peers.iter().map(|p| p.total_vectors).sum();
    let elapsed = start.elapsed().as_millis() as u64;

    let report = MeshReport {
        total_peers: peers.len(),
        healthy_peers: healthy,
        total_vectors_cluster: total_vecs,
        scan_duration_ms: elapsed,
        peers,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    };

    Ok(report)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    eprintln!("[cog-swarm-mesh-manager] starting (interval={interval}s)");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_topology(&report) {
                    eprintln!("[cog-swarm-mesh-manager] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-swarm-mesh-manager] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
