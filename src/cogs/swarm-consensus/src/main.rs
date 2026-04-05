//! Cognitum Cog: Swarm Consensus
//!
//! Multi-seed voting protocol. Propose changes, collect votes from peers,
//! commit on majority. Simple 2-phase commit (prepare + commit).
//!
//! Usage:
//!   cog-swarm-consensus --once --peers 169.254.42.2,169.254.42.3 --propose "enable-cog:presence"
//!   cog-swarm-consensus --interval 60 --peers 169.254.42.2

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);

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

/// Simple proposal ID from content hash
fn proposal_id(content: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in content.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(serde::Serialize)]
struct Vote {
    peer: String,
    vote: String,  // "prepare-ok", "prepare-reject", "error"
    reason: Option<String>,
}

#[derive(serde::Serialize)]
struct ConsensusReport {
    proposal: String,
    proposal_id: u64,
    phase: String,
    votes: Vec<Vote>,
    total_votes: usize,
    approvals: usize,
    rejections: usize,
    quorum_reached: bool,
    committed: bool,
    timestamp: u64,
}

fn run_proposal(peers: &[String], proposal: &str) -> Result<ConsensusReport, String> {
    let pid = proposal_id(proposal);
    let total_nodes = peers.len() + 1; // including self
    let quorum = total_nodes / 2 + 1;

    // Phase 1: PREPARE — ask all peers to prepare
    let mut votes = Vec::new();
    let prepare_payload = serde_json::json!({
        "type": "prepare",
        "proposal_id": pid,
        "proposal": proposal,
        "proposer": "local"
    });
    let prepare_body = serde_json::to_vec(&prepare_payload).unwrap_or_default();

    // Self always votes yes
    votes.push(Vote {
        peer: "127.0.0.1".to_string(),
        vote: "prepare-ok".to_string(),
        reason: None,
    });

    for peer in peers {
        let addr = format!("{peer}:80");
        match http_post(&addr, "/api/v1/consensus/prepare", &prepare_body) {
            Ok(resp) => {
                let vote_str = resp.get("vote").and_then(|v| v.as_str())
                    .unwrap_or("prepare-ok").to_string();
                votes.push(Vote {
                    peer: peer.clone(),
                    vote: vote_str,
                    reason: resp.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }
            Err(e) => {
                // Unreachable peers are treated as rejection (fail-safe)
                votes.push(Vote {
                    peer: peer.clone(),
                    vote: "error".to_string(),
                    reason: Some(e),
                });
            }
        }
    }

    let approvals = votes.iter().filter(|v| v.vote == "prepare-ok").count();
    let rejections = votes.len() - approvals;
    let quorum_reached = approvals >= quorum;

    // Phase 2: COMMIT if quorum reached
    let committed = if quorum_reached {
        let commit_payload = serde_json::json!({
            "type": "commit",
            "proposal_id": pid,
            "proposal": proposal
        });
        let commit_body = serde_json::to_vec(&commit_payload).unwrap_or_default();

        // Commit to self
        let _ = http_post("127.0.0.1:80", "/api/v1/consensus/commit", &commit_body);

        // Commit to peers that voted ok
        for vote in &votes {
            if vote.vote == "prepare-ok" && vote.peer != "127.0.0.1" {
                let addr = format!("{}:80", vote.peer);
                let _ = http_post(&addr, "/api/v1/consensus/commit", &commit_body);
            }
        }
        true
    } else {
        // Abort — send rollback
        let abort_payload = serde_json::json!({
            "type": "abort",
            "proposal_id": pid,
            "proposal": proposal
        });
        let abort_body = serde_json::to_vec(&abort_payload).unwrap_or_default();
        for peer in peers {
            let addr = format!("{peer}:80");
            let _ = http_post(&addr, "/api/v1/consensus/abort", &abort_body);
        }
        false
    };

    // Store consensus result as vector
    let vector = vec![
        if committed { 1.0 } else { 0.0 },
        approvals as f64 / total_nodes as f64,
        rejections as f64 / total_nodes as f64,
        total_nodes as f64 / 255.0,
        pid as f64 / u64::MAX as f64,
        0.0, 0.0, 0.0,
    ];
    let store = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let store_body = serde_json::to_vec(&store).unwrap_or_default();
    let _ = http_post("127.0.0.1:80", "/api/v1/store/ingest", &store_body);

    Ok(ConsensusReport {
        proposal: proposal.to_string(),
        proposal_id: pid,
        phase: if committed { "committed" } else { "aborted" }.to_string(),
        total_votes: votes.len(),
        approvals,
        rejections,
        quorum_reached,
        committed,
        votes,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn run_heartbeat(peers: &[String]) -> Result<ConsensusReport, String> {
    run_proposal(peers, "heartbeat-ping")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);
    let peers: Vec<String> = args.iter()
        .position(|a| a == "--peers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();
    let proposal = args.iter()
        .position(|a| a == "--propose")
        .and_then(|i| args.get(i + 1))
        .cloned();

    eprintln!("[cog-swarm-consensus] starting (peers={}, interval={interval}s)", peers.len());

    if let Some(prop) = &proposal {
        // One-shot proposal
        match run_proposal(&peers, prop) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if !report.committed {
                    eprintln!("[cog-swarm-consensus] ALERT: proposal rejected (quorum not reached)");
                }
            }
            Err(e) => eprintln!("[cog-swarm-consensus] error: {e}"),
        }
        return;
    }

    // Continuous heartbeat mode
    loop {
        let start = Instant::now();
        match run_heartbeat(&peers) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-swarm-consensus] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
