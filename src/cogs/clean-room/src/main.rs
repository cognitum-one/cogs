//! Cognitum Cog: Clean Room Headcount Enforcement
//!
//! Counts simultaneous presences via independent signal clusters.
//! Alerts when headcount exceeds configurable limit.
//!
//! Usage:
//!   cog-clean-room --once --max-occupancy 4
//!   cog-clean-room --interval 3 --max-occupancy 6

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
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

fn store_vector(vec: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec.to_vec()]], "dedup": true });
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

/// Estimate occupancy from channel signal patterns.
/// Uses a simple clustering approach: channels with correlated signal
/// deviations likely correspond to the same person. Independent
/// deviations suggest different individuals.
fn estimate_occupancy(values: &[f64]) -> usize {
    if values.is_empty() { return 0; }

    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let active: Vec<f64> = values.iter()
        .filter(|&&v| (v - mean).abs() > 1.0) // Only channels showing activity
        .copied()
        .collect();

    if active.is_empty() { return 0; }

    // Simple cluster estimation via gap analysis
    // Sort active signal strengths and count distinct clusters
    let mut sorted = active.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters = 1usize;
    let cluster_gap = 5.0; // Minimum signal gap between clusters

    for i in 1..sorted.len() {
        if (sorted[i] - sorted[i - 1]).abs() > cluster_gap {
            clusters += 1;
        }
    }

    clusters
}

struct OccupancyState {
    history: Vec<usize>,
    max_history: usize,
}

impl OccupancyState {
    fn new() -> Self {
        Self { history: Vec::new(), max_history: 10 }
    }

    fn add(&mut self, count: usize) -> usize {
        self.history.push(count);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
        // Median filter for stability
        let mut sorted = self.history.clone();
        sorted.sort();
        sorted[sorted.len() / 2]
    }
}

#[derive(serde::Serialize)]
struct CleanRoomReport {
    estimated_occupancy: usize,
    max_occupancy: usize,
    over_limit: bool,
    headroom: i32,
    alert: bool,
    signal_clusters: usize,
    active_channels: usize,
    total_channels: usize,
    timestamp: u64,
}

fn run_once(state: &mut OccupancyState, max_occupancy: usize) -> Result<CleanRoomReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    let raw_count = estimate_occupancy(&values);
    let smoothed = state.add(raw_count);

    let mean: f64 = if values.is_empty() { 0.0 } else {
        values.iter().sum::<f64>() / values.len() as f64
    };
    let active = values.iter().filter(|&&v| (v - mean).abs() > 1.0).count();

    let over_limit = smoothed > max_occupancy;
    let headroom = max_occupancy as i32 - smoothed as i32;

    let vector = [
        smoothed as f64 / max_occupancy.max(1) as f64,
        if over_limit { 1.0 } else { 0.0 },
        headroom as f64 / max_occupancy.max(1) as f64,
        active as f64 / values.len().max(1) as f64,
        raw_count as f64 / 20.0,
        0.0, 0.0, 0.0,
    ];
    let _ = store_vector(&vector);

    Ok(CleanRoomReport {
        estimated_occupancy: smoothed,
        max_occupancy,
        over_limit,
        headroom,
        alert: over_limit,
        signal_clusters: raw_count,
        active_channels: active,
        total_channels: values.len(),
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
        .unwrap_or(3);
    let max_occupancy = args.iter()
        .position(|a| a == "--max-occupancy")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    eprintln!("[cog-clean-room] starting (max_occupancy={max_occupancy}, interval={interval}s)");
    let mut state = OccupancyState::new();

    loop {
        let start = Instant::now();
        match run_once(&mut state, max_occupancy) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.alert {
                    eprintln!("[cog-clean-room] ALERT: Occupancy {} exceeds limit {}", report.estimated_occupancy, report.max_occupancy);
                }
            }
            Err(e) => eprintln!("[cog-clean-room] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
