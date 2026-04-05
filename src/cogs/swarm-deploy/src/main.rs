//! Cognitum Cog: Swarm Deploy
//!
//! Install/uninstall cogs on peer seeds via POST /api/v1/apps/install
//! and DELETE /api/v1/apps/{id}. Fan-out to all peers, report status.
//!
//! Usage:
//!   cog-swarm-deploy --once --peers 169.254.42.2,169.254.42.3 --install cog-presence
//!   cog-swarm-deploy --once --peers 169.254.42.2 --uninstall cog-presence

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

fn http_request(addr: &str, method: &str, path: &str, body: Option<&[u8]>) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TIMEOUT)).ok();

    if let Some(body) = body {
        write!(conn, "{method} {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
            .map_err(|e| format!("write: {e}"))?;
        conn.write_all(body).map_err(|e| format!("body: {e}"))?;
    } else {
        write!(conn, "{method} {path} HTTP/1.0\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
            .map_err(|e| format!("write: {e}"))?;
    }

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
    let resp = String::from_utf8_lossy(&buf);

    // Extract status code
    let status_code = resp.lines().next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    if let Some(json_start) = resp.find('{') {
        serde_json::from_str(&resp[json_start..]).map_err(|e| format!("parse: {e}"))
    } else {
        Ok(serde_json::json!({ "status_code": status_code }))
    }
}

#[derive(serde::Serialize)]
struct DeployResult {
    peer: String,
    action: String,
    cog_id: String,
    success: bool,
    response: serde_json::Value,
    duration_ms: u64,
}

#[derive(serde::Serialize)]
struct DeployReport {
    action: String,
    cog_id: String,
    results: Vec<DeployResult>,
    total_peers: usize,
    successful: usize,
    failed: usize,
    timestamp: u64,
}

fn deploy_to_peer(peer: &str, action: &str, cog_id: &str) -> DeployResult {
    let addr = format!("{peer}:80");
    let start = Instant::now();

    let (method, path, body) = match action {
        "install" => {
            let payload = serde_json::json!({
                "id": cog_id,
                "name": cog_id,
                "source": "swarm-deploy"
            });
            let body = serde_json::to_vec(&payload).unwrap_or_default();
            ("POST", "/api/v1/apps/install".to_string(), Some(body))
        }
        "uninstall" => {
            ("DELETE", format!("/api/v1/apps/{cog_id}"), None)
        }
        _ => {
            return DeployResult {
                peer: peer.to_string(),
                action: action.to_string(),
                cog_id: cog_id.to_string(),
                success: false,
                response: serde_json::json!({"error": "unknown action"}),
                duration_ms: 0,
            };
        }
    };

    match http_request(&addr, method, &path, body.as_deref()) {
        Ok(resp) => DeployResult {
            peer: peer.to_string(),
            action: action.to_string(),
            cog_id: cog_id.to_string(),
            success: true,
            response: resp,
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => DeployResult {
            peer: peer.to_string(),
            action: action.to_string(),
            cog_id: cog_id.to_string(),
            success: false,
            response: serde_json::json!({"error": e}),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let peers: Vec<String> = args.iter()
        .position(|a| a == "--peers")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    let install_cog = args.iter()
        .position(|a| a == "--install")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let uninstall_cog = args.iter()
        .position(|a| a == "--uninstall")
        .and_then(|i| args.get(i + 1))
        .cloned();

    // F-14: Sanitize cog_id
    let validate_id = |id: &str| -> bool {
        !id.is_empty() && id.len() < 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };

    let (action, cog_id) = if let Some(id) = install_cog {
        if !validate_id(&id) { eprintln!("[cog-swarm-deploy] error: invalid cog id"); std::process::exit(1); }
        ("install", id)
    } else if let Some(id) = uninstall_cog {
        if !validate_id(&id) { eprintln!("[cog-swarm-deploy] error: invalid cog id"); std::process::exit(1); }
        ("uninstall", id)
    } else {
        // Status mode: report deploy readiness
        let status = serde_json::json!({
            "status": "ready",
            "peers_configured": peers.len(),
            "peers": peers,
            "usage": "specify --install <cog-id> or --uninstall <cog-id>",
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        println!("{}", status);
        return;
    };

    if peers.is_empty() {
        eprintln!("[cog-swarm-deploy] error: specify --peers <ip1,ip2,...>");
        std::process::exit(1);
    }

    eprintln!("[cog-swarm-deploy] {action} {cog_id} on {} peers", peers.len());

    let results: Vec<DeployResult> = peers.iter()
        .map(|p| deploy_to_peer(p, action, &cog_id))
        .collect();

    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;

    let report = DeployReport {
        action: action.to_string(),
        cog_id: cog_id.clone(),
        total_peers: results.len(),
        successful,
        failed,
        results,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    };

    println!("{}", serde_json::to_string(&report).unwrap_or_default());

    if failed > 0 {
        eprintln!("[cog-swarm-deploy] ALERT: {failed} peers failed deployment");
    }

    if !once {
        eprintln!("[cog-swarm-deploy] deploy is a one-shot operation, exiting");
    }
}
