//! Cognitum Cog: Self-Healing Mesh
//!
//! Monitor sensor mesh health. Detect dropped nodes (channels going silent).
//! Redistribute processing to remaining channels. Report mesh topology status.
//!
//! Usage:
//!   cog-self-healing-mesh --once
//!   cog-self-healing-mesh --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;
const SILENCE_THRESHOLD: f64 = 0.001;
const HISTORY_WINDOW: usize = 10;

/// Node health tracker
struct NodeHealth {
    channel: String,
    active_history: Vec<bool>, // ring buffer of active/inactive
    last_value: f64,
    consecutive_silent: u32,
    total_active: u64,
    total_checks: u64,
}

impl NodeHealth {
    fn new(channel: &str) -> Self {
        Self {
            channel: channel.into(),
            active_history: Vec::new(),
            last_value: 0.0,
            consecutive_silent: 0,
            total_active: 0,
            total_checks: 0,
        }
    }

    fn update(&mut self, values: &[f64]) {
        self.total_checks += 1;
        let is_active = values.iter().any(|v| v.abs() > SILENCE_THRESHOLD);
        self.last_value = values.last().cloned().unwrap_or(0.0);

        if is_active {
            self.consecutive_silent = 0;
            self.total_active += 1;
        } else {
            self.consecutive_silent += 1;
        }

        self.active_history.push(is_active);
        if self.active_history.len() > HISTORY_WINDOW {
            self.active_history.remove(0);
        }
    }

    fn uptime_ratio(&self) -> f64 {
        if self.total_checks == 0 { return 1.0; }
        self.total_active as f64 / self.total_checks as f64
    }

    fn recent_health(&self) -> f64 {
        if self.active_history.is_empty() { return 1.0; }
        self.active_history.iter().filter(|&&a| a).count() as f64
            / self.active_history.len() as f64
    }

    fn is_dropped(&self) -> bool {
        self.consecutive_silent >= 3
    }

    fn status(&self) -> &str {
        if self.is_dropped() { "dropped" }
        else if self.recent_health() < 0.5 { "degraded" }
        else { "healthy" }
    }
}

/// Mesh topology manager
struct MeshTopology {
    nodes: std::collections::HashMap<String, NodeHealth>,
}

impl MeshTopology {
    fn new() -> Self {
        Self { nodes: std::collections::HashMap::new() }
    }

    fn update(&mut self, channels: &std::collections::HashMap<String, Vec<f64>>) {
        // Update existing and add new nodes
        for (ch, vals) in channels {
            self.nodes.entry(ch.clone())
                .or_insert_with(|| NodeHealth::new(ch))
                .update(vals);
        }

        // Mark channels not in current data as receiving empty
        let current_channels: std::collections::HashSet<&String> = channels.keys().collect();
        for (ch, node) in &mut self.nodes {
            if !current_channels.contains(ch) {
                node.update(&[]);
            }
        }
    }

    fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    fn active_nodes(&self) -> usize {
        self.nodes.values().filter(|n| !n.is_dropped()).count()
    }

    fn dropped_nodes(&self) -> Vec<&str> {
        self.nodes.iter()
            .filter(|(_, n)| n.is_dropped())
            .map(|(ch, _)| ch.as_str())
            .collect()
    }

    fn mesh_health(&self) -> f64 {
        if self.nodes.is_empty() { return 1.0; }
        self.nodes.values().map(|n| n.recent_health()).sum::<f64>()
            / self.nodes.len() as f64
    }

    /// Compute load redistribution weights for active nodes
    fn redistribution_weights(&self) -> std::collections::HashMap<String, f64> {
        let active: Vec<(&String, &NodeHealth)> = self.nodes.iter()
            .filter(|(_, n)| !n.is_dropped())
            .collect();
        if active.is_empty() {
            return std::collections::HashMap::new();
        }

        let total = self.nodes.len() as f64;
        let active_count = active.len() as f64;
        let weight = total / active_count; // Each active node takes proportionally more load

        active.iter().map(|(ch, n)| {
            let health_factor = n.recent_health();
            ((*ch).clone(), weight * health_factor)
        }).collect()
    }
}

#[derive(serde::Serialize)]
struct MeshResult {
    total_nodes: usize,
    active_nodes: usize,
    dropped_nodes: Vec<String>,
    mesh_health: f64,
    mesh_status: String,
    redistribution: std::collections::HashMap<String, f64>,
    node_statuses: Vec<NodeStatus>,
    anomalies: Vec<String>,
    vector: [f64; DIM],
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct NodeStatus {
    channel: String,
    status: String,
    uptime: f64,
    recent_health: f64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
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
    let start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(v: &[f64; DIM]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once(mesh: &mut MeshTopology) -> Result<MeshResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    mesh.update(&channels);

    let total = mesh.total_nodes();
    let active = mesh.active_nodes();
    let dropped: Vec<String> = mesh.dropped_nodes().iter().map(|s| s.to_string()).collect();
    let health = mesh.mesh_health();
    let redistribution = mesh.redistribution_weights();

    let mesh_status = if health > 0.9 { "healthy" }
        else if health > 0.6 { "degraded" }
        else if health > 0.3 { "critical" }
        else { "failing" };

    let node_statuses: Vec<NodeStatus> = mesh.nodes.iter().map(|(ch, n)| {
        NodeStatus {
            channel: ch.clone(),
            status: n.status().into(),
            uptime: n.uptime_ratio(),
            recent_health: n.recent_health(),
        }
    }).collect();

    let mut anomalies = Vec::new();
    if !dropped.is_empty() {
        anomalies.push(format!("NODES_DROPPED: {:?}", dropped));
    }
    if health < 0.5 {
        anomalies.push(format!("MESH_DEGRADED: health={health:.2}"));
    }
    if active < total / 2 {
        anomalies.push(format!("MAJORITY_OFFLINE: {active}/{total} active"));
    }

    let vector = [
        health,
        active as f64 / total.max(1) as f64,
        dropped.len() as f64 / total.max(1) as f64,
        total as f64 / 10.0,
        if mesh_status == "healthy" { 1.0 } else { 0.0 },
        node_statuses.iter().map(|n| n.uptime).sum::<f64>() / node_statuses.len().max(1) as f64,
        if anomalies.is_empty() { 0.0 } else { 1.0 },
        0.0,
    ];

    let _ = store_vector(&vector);

    Ok(MeshResult {
        total_nodes: total,
        active_nodes: active,
        dropped_nodes: dropped,
        mesh_health: health,
        mesh_status: mesh_status.into(),
        redistribution,
        node_statuses,
        anomalies,
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-self-healing-mesh] starting (interval={interval}s, once={once})");

    let mut mesh = MeshTopology::new();

    loop {
        let start = Instant::now();
        match run_once(&mut mesh) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-self-healing-mesh] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-self-healing-mesh] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
