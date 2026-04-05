//! Cognitum Cog: Occupancy Zones
//!
//! Counts people per zone using per-channel signal variance.
//! Each sensor channel = one zone. Presence is detected when
//! variance exceeds a calibrated threshold.
//!
//! Usage:
//!   cog-occupancy-zones --once
//!   cog-occupancy-zones --interval 5

use std::collections::HashMap;
use std::io::Read;
use std::time::{Duration, Instant};

struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordStats {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }
    fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }
}

struct ZoneTracker {
    baselines: HashMap<String, WelfordStats>,
    variance_threshold: f64,
}

impl ZoneTracker {
    fn new(threshold: f64) -> Self {
        Self { baselines: HashMap::new(), variance_threshold: threshold }
    }

    fn process(&mut self, samples: &[(String, f64)]) -> ZoneReport {
        let mut zone_counts: HashMap<String, u32> = HashMap::new();
        let mut zone_variances: HashMap<String, f64> = HashMap::new();

        // Group samples by channel and compute variance per zone
        let mut channel_samples: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_samples.entry(ch.clone()).or_default().push(*val);
        }

        let mut occupied_zones = Vec::new();
        for (ch, vals) in &channel_samples {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();
            zone_variances.insert(ch.clone(), var);

            let baseline = self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new);
            let is_present = var > self.variance_threshold;

            if is_present {
                *zone_counts.entry(ch.clone()).or_insert(0) += 1;
                occupied_zones.push(ch.clone());
            } else {
                baseline.update(var);
            }
        }

        let total_zones = channel_samples.len() as u32;
        let occupied = occupied_zones.len() as u32;

        ZoneReport {
            total_zones,
            occupied_zones: occupied,
            zone_details: occupied_zones.iter().map(|z| {
                let var = zone_variances.get(z).copied().unwrap_or(0.0);
                ZoneDetail { zone: z.clone(), variance: var, present: true }
            }).collect(),
            occupancy_ratio: if total_zones > 0 { occupied as f64 / total_zones as f64 } else { 0.0 },
            timestamp: now_secs(),
        }
    }
}

#[derive(serde::Serialize)]
struct ZoneDetail {
    zone: String,
    variance: f64,
    present: bool,
}

#[derive(serde::Serialize)]
struct ZoneReport {
    total_zones: u32,
    occupied_zones: u32,
    zone_details: Vec<ZoneDetail>,
    occupancy_ratio: f64,
    timestamp: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON in response")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(report: &ZoneReport) -> Result<(), String> {
    let vector = vec![
        report.occupied_zones as f64,
        report.total_zones as f64,
        report.occupancy_ratio,
        0.0, 0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
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

fn run_once(tracker: &mut ZoneTracker) -> Result<ZoneReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let parsed: Vec<(String, f64)> = samples.iter().filter_map(|s| {
        let ch = s.get("channel")?.as_str()?.to_string();
        let val = s.get("value")?.as_f64()?;
        Some((ch, val))
    }).collect();

    if parsed.is_empty() {
        return Err("no sensor data".into());
    }

    let report = tracker.process(&parsed);

    if report.occupied_zones > 0 {
        eprintln!("[cog-occupancy-zones] ALERT: {}/{} zones occupied",
            report.occupied_zones, report.total_zones);
    }

    Ok(report)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5);

    eprintln!("[cog-occupancy-zones] starting (interval={}s)", interval);
    let mut tracker = ZoneTracker::new(10.0);

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-occupancy-zones] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-occupancy-zones] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
