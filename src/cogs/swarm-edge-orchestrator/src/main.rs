//! Cognitum Cog: Swarm Edge Orchestrator
//!
//! Manages ESP32 sensor nodes. Queries local UDP port 5005 for CSI data.
//! Configures ESP32 parameters via serial proxy. Stores sensor readings.
//!
//! Usage:
//!   cog-swarm-edge-orchestrator --once
//!   cog-swarm-edge-orchestrator --interval 2 --udp-port 5005

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::time::{Duration, Instant};

const HTTP_TIMEOUT: Duration = Duration::from_secs(3);
const UDP_TIMEOUT: Duration = Duration::from_millis(500);

fn http_post(addr: &str, path: &str, payload: &[u8]) -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("addr: {e}"))?,
        HTTP_TIMEOUT,
    ).map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(HTTP_TIMEOUT)).ok();
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CsiFrame {
    mac: String,
    rssi: i32,
    channel: u8,
    subcarriers: Vec<f64>,
    timestamp_us: u64,
}

#[derive(serde::Serialize)]
struct EdgeReport {
    esp32_nodes: Vec<NodeStatus>,
    total_frames: u64,
    avg_rssi: f64,
    channels_active: Vec<u8>,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct NodeStatus {
    mac: String,
    rssi: i32,
    channel: u8,
    subcarrier_count: usize,
    last_seen_ms: u64,
}

fn read_csi_frames(udp_port: u16) -> Vec<CsiFrame> {
    let bind_addr = format!("127.0.0.1:{udp_port}");
    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[cog-swarm-edge-orchestrator] UDP bind error: {e}");
            return Vec::new();
        }
    };
    socket.set_read_timeout(Some(UDP_TIMEOUT)).ok();

    let mut frames = Vec::new();
    let mut buf = [0u8; 4096];

    // Collect frames for a short window
    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((len, _src)) => {
                let data = &buf[..len];
                // Try to parse as JSON CSI frame
                if let Ok(frame) = serde_json::from_slice::<CsiFrame>(data) {
                    frames.push(frame);
                } else {
                    // Try raw binary format: [MAC:6][RSSI:1][CH:1][N_SUB:2][SUB:N*4]
                    if len >= 10 {
                        let mac = format!(
                            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                            data[0], data[1], data[2], data[3], data[4], data[5]
                        );
                        let rssi = data[6] as i8 as i32;
                        let channel = data[7];
                        let n_sub = u16::from_le_bytes([data[8], data[9]]) as usize;
                        let mut subcarriers = Vec::with_capacity(n_sub);
                        for i in 0..n_sub {
                            let offset = 10 + i * 4;
                            if offset + 4 <= len {
                                let val = f32::from_le_bytes([
                                    data[offset], data[offset+1], data[offset+2], data[offset+3]
                                ]);
                                subcarriers.push(val as f64);
                            }
                        }
                        frames.push(CsiFrame {
                            mac,
                            rssi,
                            channel,
                            subcarriers,
                            timestamp_us: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default().as_micros() as u64,
                        });
                    }
                }
            }
            Err(_) => break,
        }
    }
    frames
}

fn store_csi_vector(frames: &[CsiFrame]) -> Result<(), String> {
    if frames.is_empty() { return Ok(()); }

    // Aggregate subcarriers across frames into an 8-dim feature vector
    let mut agg = [0.0f64; 8];
    let mut count = 0u64;
    for frame in frames {
        for (i, &val) in frame.subcarriers.iter().take(8).enumerate() {
            agg[i] += val;
        }
        count += 1;
    }
    if count > 0 {
        for v in &mut agg { *v /= count as f64; }
    }

    let payload = serde_json::json!({ "vectors": [[0, agg.to_vec()]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    http_post("127.0.0.1:80", "/api/v1/store/ingest", &body)?;
    Ok(())
}

fn run_once(udp_port: u16) -> Result<EdgeReport, String> {
    let frames = read_csi_frames(udp_port);

    // Build node status from frames
    let mut nodes: std::collections::HashMap<String, NodeStatus> = std::collections::HashMap::new();
    for frame in &frames {
        let entry = nodes.entry(frame.mac.clone()).or_insert(NodeStatus {
            mac: frame.mac.clone(),
            rssi: frame.rssi,
            channel: frame.channel,
            subcarrier_count: frame.subcarriers.len(),
            last_seen_ms: 0,
        });
        entry.rssi = frame.rssi;
        entry.channel = frame.channel;
        entry.subcarrier_count = frame.subcarriers.len();
    }

    let avg_rssi = if frames.is_empty() { 0.0 } else {
        frames.iter().map(|f| f.rssi as f64).sum::<f64>() / frames.len() as f64
    };

    let mut channels: Vec<u8> = frames.iter().map(|f| f.channel).collect();
    channels.sort();
    channels.dedup();

    // Store aggregated CSI data
    if let Err(e) = store_csi_vector(&frames) {
        eprintln!("[cog-swarm-edge-orchestrator] store error: {e}");
    }

    Ok(EdgeReport {
        esp32_nodes: nodes.into_values().collect(),
        total_frames: frames.len() as u64,
        avg_rssi,
        channels_active: channels,
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
        .unwrap_or(2);
    let udp_port = args.iter()
        .position(|a| a == "--udp-port")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(5005);

    eprintln!("[cog-swarm-edge-orchestrator] starting (udp_port={udp_port}, interval={interval}s)");

    loop {
        let start = Instant::now();
        match run_once(udp_port) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-swarm-edge-orchestrator] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
