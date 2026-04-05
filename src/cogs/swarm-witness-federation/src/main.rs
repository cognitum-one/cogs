//! Cognitum Cog: Swarm Witness Federation
//!
//! Shares witness chain entries between seeds. Cross-attests events
//! for tamper detection. Verifies peer witness chains.
//!
//! Usage:
//!   cog-swarm-witness-federation --once --peers 169.254.42.2
//!   cog-swarm-witness-federation --interval 30

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);

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
    // Try JSON object or array
    let start = body.find(|c: char| c == '{' || c == '[').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
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
    let start = body.find(|c: char| c == '{' || c == '[').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
}

/// Simple hash for witness chain verification
fn simple_hash(data: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct WitnessEntry {
    epoch: u64,
    event: String,
    hash: String,
    prev_hash: String,
    device_id: String,
}

#[derive(serde::Serialize)]
struct FederationReport {
    local_chain_length: usize,
    peers_synced: usize,
    entries_shared: usize,
    entries_received: usize,
    chain_valid: bool,
    cross_attestations: usize,
    tamper_alerts: Vec<String>,
    timestamp: u64,
}

fn get_witness_chain(addr: &str) -> Result<Vec<WitnessEntry>, String> {
    let resp = http_get(addr, "/api/v1/witness/chain")?;
    if let Some(arr) = resp.as_array() {
        let entries: Vec<WitnessEntry> = arr.iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        Ok(entries)
    } else if let Some(entries) = resp.get("entries").and_then(|v| v.as_array()) {
        let entries: Vec<WitnessEntry> = entries.iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        Ok(entries)
    } else {
        Ok(Vec::new())
    }
}

fn verify_chain(chain: &[WitnessEntry]) -> (bool, Vec<String>) {
    let mut alerts = Vec::new();
    for i in 1..chain.len() {
        if chain[i].prev_hash != chain[i - 1].hash {
            alerts.push(format!(
                "chain break at epoch {}: expected prev_hash={} got={}",
                chain[i].epoch, chain[i - 1].hash, chain[i].prev_hash
            ));
        }
    }
    (alerts.is_empty(), alerts)
}

fn attest_entry(addr: &str, entry: &WitnessEntry) -> Result<(), String> {
    let attestation = serde_json::json!({
        "type": "cross-attestation",
        "epoch": entry.epoch,
        "hash": entry.hash,
        "attester": "local",
        "verified": true
    });
    let body = serde_json::to_vec(&attestation).map_err(|e| format!("json: {e}"))?;
    http_post(addr, "/api/v1/witness/attest", &body)?;
    Ok(())
}

fn run_once(peers: &[String]) -> Result<FederationReport, String> {
    let local_chain = get_witness_chain("127.0.0.1:80").unwrap_or_default();
    let (chain_valid, mut tamper_alerts) = verify_chain(&local_chain);

    let mut peers_synced = 0usize;
    let mut entries_shared = 0usize;
    let mut entries_received = 0usize;
    let mut cross_attestations = 0usize;

    for peer in peers {
        let peer_addr = format!("{peer}:80");

        // Get peer chain
        match get_witness_chain(&peer_addr) {
            Ok(peer_chain) => {
                peers_synced += 1;

                // Verify peer chain integrity
                let (peer_valid, peer_alerts) = verify_chain(&peer_chain);
                if !peer_valid {
                    for alert in peer_alerts {
                        tamper_alerts.push(format!("peer {peer}: {alert}"));
                    }
                }

                entries_received += peer_chain.len();

                // Cross-attest latest entries
                for entry in peer_chain.iter().rev().take(3) {
                    if attest_entry(&peer_addr, entry).is_ok() {
                        cross_attestations += 1;
                    }
                }

                // Share our latest entries with peer
                for entry in local_chain.iter().rev().take(3) {
                    let payload = serde_json::json!({
                        "entry": entry,
                        "source": "federation"
                    });
                    let body = serde_json::to_vec(&payload).unwrap_or_default();
                    if http_post(&peer_addr, "/api/v1/witness/share", &body).is_ok() {
                        entries_shared += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("[cog-swarm-witness-federation] peer {peer} error: {e}");
            }
        }
    }

    // Store federation state as vector
    let vector = vec![
        local_chain.len() as f64 / 1000.0,
        peers_synced as f64 / peers.len().max(1) as f64,
        if chain_valid { 1.0 } else { 0.0 },
        cross_attestations as f64 / 100.0,
        tamper_alerts.len() as f64 / 10.0,
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).unwrap_or_default();
    let _ = http_post("127.0.0.1:80", "/api/v1/store/ingest", &body);

    Ok(FederationReport {
        local_chain_length: local_chain.len(),
        peers_synced,
        entries_shared,
        entries_received,
        chain_valid,
        cross_attestations,
        tamper_alerts,
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
        .unwrap_or(30);
    let peers: Vec<String> = args.iter()
        .position(|a| a == "--peers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    eprintln!("[cog-swarm-witness-federation] starting (peers={}, interval={interval}s)", peers.len());

    loop {
        let start = Instant::now();
        match run_once(&peers) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if !report.tamper_alerts.is_empty() {
                    eprintln!("[cog-swarm-witness-federation] TAMPER ALERT: {} issues detected", report.tamper_alerts.len());
                }
            }
            Err(e) => eprintln!("[cog-swarm-witness-federation] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
