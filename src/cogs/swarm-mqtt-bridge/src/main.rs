//! Cognitum Cog: Swarm MQTT Bridge
//!
//! Lightweight TCP-based pub/sub event sharing between seeds.
//! Publishes local events to peers, subscribes to peer topics.
//! Protocol: newline-delimited JSON over TCP port 1883.
//!
//! Usage:
//!   cog-swarm-mqtt-bridge --once --peers 169.254.42.2
//!   cog-swarm-mqtt-bridge --interval 5 --peers 169.254.42.2,169.254.42.3

use std::io::{Read, Write, BufRead, BufReader};
use std::net::{TcpStream, TcpListener};
use std::time::{Duration, Instant};

const TCP_TIMEOUT: Duration = Duration::from_secs(2);
const BRIDGE_PORT: u16 = 1883;

fn http_get(addr: &str, path: &str) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TCP_TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TCP_TIMEOUT)).ok();
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

fn http_post(addr: &str, path: &str, payload: &[u8]) -> Result<(), String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TCP_TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(TCP_TIMEOUT)).ok();
    write!(conn, "POST {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", payload.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(payload).map_err(|e| format!("body: {e}"))?;
    let mut buf = Vec::new();
    conn.read_to_end(&mut buf).ok();
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct MqttMessage {
    topic: String,
    payload: serde_json::Value,
    sender: String,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct BridgeReport {
    messages_published: usize,
    messages_received: usize,
    peers_connected: usize,
    topics: Vec<String>,
    timestamp: u64,
}

fn publish_to_peer(peer: &str, msg: &MqttMessage) -> Result<(), String> {
    let addr = format!("{peer}:{BRIDGE_PORT}");
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        TCP_TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_write_timeout(Some(TCP_TIMEOUT)).ok();
    let line = serde_json::to_string(msg).map_err(|e| format!("json: {e}"))?;
    writeln!(conn, "{line}").map_err(|e| format!("write: {e}"))?;
    Ok(())
}

fn collect_local_events() -> Vec<MqttMessage> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs();

    let mut events = Vec::new();

    // Collect sensor data as an event
    if let Ok(sensors) = http_get("127.0.0.1:80", "/api/v1/sensor/stream") {
        events.push(MqttMessage {
            topic: "sensor/stream".to_string(),
            payload: sensors,
            sender: "local".to_string(),
            timestamp: now,
        });
    }

    // Collect status as an event
    if let Ok(status) = http_get("127.0.0.1:80", "/api/v1/status") {
        events.push(MqttMessage {
            topic: "status/heartbeat".to_string(),
            payload: status,
            sender: "local".to_string(),
            timestamp: now,
        });
    }

    events
}

fn receive_from_peer(peer: &str) -> Vec<MqttMessage> {
    let addr = format!("{peer}:{BRIDGE_PORT}");
    let conn = match TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
        TCP_TIMEOUT,
    ) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    conn.set_read_timeout(Some(Duration::from_millis(500))).ok();

    let reader = BufReader::new(conn);
    let mut messages = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(l) if !l.is_empty() => {
                if let Ok(msg) = serde_json::from_str::<MqttMessage>(&l) {
                    messages.push(msg);
                }
            }
            _ => break,
        }
    }
    messages
}

fn run_once(peers: &[String]) -> Result<BridgeReport, String> {
    let local_events = collect_local_events();
    let mut published = 0usize;
    let mut received = 0usize;
    let mut connected = 0usize;
    let mut all_topics: Vec<String> = Vec::new();

    // Publish local events to all peers
    for peer in peers {
        let mut peer_ok = false;
        for event in &local_events {
            if publish_to_peer(peer, event).is_ok() {
                published += 1;
                peer_ok = true;
                if !all_topics.contains(&event.topic) {
                    all_topics.push(event.topic.clone());
                }
            }
        }
        if peer_ok { connected += 1; }

        // Receive events from peer
        let peer_messages = receive_from_peer(peer);
        for msg in &peer_messages {
            if !all_topics.contains(&msg.topic) {
                all_topics.push(msg.topic.clone());
            }
            // Store received sensor data locally
            if msg.topic.starts_with("sensor/") {
                if let Some(samples) = msg.payload.get("samples").and_then(|v| v.as_array()) {
                    let mut vec = [0.0f64; 8];
                    for (i, s) in samples.iter().take(8).enumerate() {
                        vec[i] = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    }
                    let payload = serde_json::json!({ "vectors": [[0, vec.to_vec()]], "dedup": true });
                    let body = serde_json::to_vec(&payload).unwrap_or_default();
                    let _ = http_post("127.0.0.1:80", "/api/v1/store/ingest", &body);
                }
            }
        }
        received += peer_messages.len();
    }

    Ok(BridgeReport {
        messages_published: published,
        messages_received: received,
        peers_connected: connected,
        topics: all_topics,
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

    eprintln!("[cog-swarm-mqtt-bridge] starting (peers={}, interval={interval}s)", peers.len());

    loop {
        let start = Instant::now();
        match run_once(&peers) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-swarm-mqtt-bridge] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
