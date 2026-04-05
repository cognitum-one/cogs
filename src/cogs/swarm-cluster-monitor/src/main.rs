//! Cognitum Cog: Swarm Cluster Monitor
//!
//! Polls peer seeds' /api/v1/status endpoints. Aggregates health metrics
//! across the cluster: total vectors, uptime, unhealthy seeds.
//!
//! Usage:
//!   cog-swarm-cluster-monitor --once --peers 169.254.42.2,169.254.42.3
//!   cog-swarm-cluster-monitor --interval 15

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

#[derive(serde::Serialize)]
struct PeerHealth {
    address: String,
    device_id: String,
    total_vectors: u64,
    uptime_secs: u64,
    healthy: bool,
    response_ms: u64,
    error: Option<String>,
}

#[derive(serde::Serialize)]
struct ClusterReport {
    peers: Vec<PeerHealth>,
    total_seeds: usize,
    healthy_seeds: usize,
    unhealthy_seeds: usize,
    total_vectors_cluster: u64,
    max_uptime_secs: u64,
    min_uptime_secs: u64,
    avg_response_ms: f64,
    timestamp: u64,
}

fn check_peer(ip: &str) -> PeerHealth {
    let addr = format!("{ip}:80");
    let start = Instant::now();
    match http_get(&addr, "/api/v1/status") {
        Ok(status) => PeerHealth {
            address: ip.to_string(),
            device_id: status.get("device_id").and_then(|v| v.as_str())
                .unwrap_or("unknown").to_string(),
            total_vectors: status.get("total_vectors").and_then(|v| v.as_u64()).unwrap_or(0),
            uptime_secs: status.get("uptime_secs").and_then(|v| v.as_u64()).unwrap_or(0),
            healthy: true,
            response_ms: start.elapsed().as_millis() as u64,
            error: None,
        },
        Err(e) => PeerHealth {
            address: ip.to_string(),
            device_id: String::new(),
            total_vectors: 0,
            uptime_secs: 0,
            healthy: false,
            response_ms: start.elapsed().as_millis() as u64,
            error: Some(e),
        },
    }
}

fn store_cluster_health(report: &ClusterReport) -> Result<(), String> {
    let vector = vec![
        report.total_seeds as f64 / 255.0,
        report.healthy_seeds as f64 / report.total_seeds.max(1) as f64,
        (report.total_vectors_cluster as f64).ln().max(0.0) / 20.0,
        report.avg_response_ms / 5000.0,
        report.max_uptime_secs as f64 / 86400.0,
        report.unhealthy_seeds as f64 / report.total_seeds.max(1) as f64,
        0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once(peers: &[String]) -> Result<ClusterReport, String> {
    // Always include self
    let mut all_ips: Vec<String> = vec!["127.0.0.1".to_string()];
    all_ips.extend(peers.iter().cloned());

    let health: Vec<PeerHealth> = all_ips.iter().map(|ip| check_peer(ip)).collect();

    let healthy = health.iter().filter(|p| p.healthy).count();
    let unhealthy = health.len() - healthy;
    let total_vecs: u64 = health.iter().map(|p| p.total_vectors).sum();
    let max_up = health.iter().map(|p| p.uptime_secs).max().unwrap_or(0);
    let min_up = health.iter().filter(|p| p.healthy).map(|p| p.uptime_secs).min().unwrap_or(0);
    let avg_resp = if !health.is_empty() {
        health.iter().map(|p| p.response_ms as f64).sum::<f64>() / health.len() as f64
    } else { 0.0 };

    let report = ClusterReport {
        total_seeds: health.len(),
        healthy_seeds: healthy,
        unhealthy_seeds: unhealthy,
        total_vectors_cluster: total_vecs,
        max_uptime_secs: max_up,
        min_uptime_secs: min_up,
        avg_response_ms: avg_resp,
        peers: health,
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
        .unwrap_or(15);
    let peers: Vec<String> = args.iter()
        .position(|a| a == "--peers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    eprintln!("[cog-swarm-cluster-monitor] starting (peers={}, interval={interval}s)", peers.len());

    loop {
        let start = Instant::now();
        match run_once(&peers) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_cluster_health(&report) {
                    eprintln!("[cog-swarm-cluster-monitor] store error: {e}");
                }
                if report.unhealthy_seeds > 0 {
                    eprintln!("[cog-swarm-cluster-monitor] ALERT: {} unhealthy seeds", report.unhealthy_seeds);
                }
            }
            Err(e) => eprintln!("[cog-swarm-cluster-monitor] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
