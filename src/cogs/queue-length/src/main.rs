//! Cognitum Cog: Queue Length
//!
//! Estimates queue by counting distinct presence zones in a line.
//! Tracks wait time from first detection to departure per position.
//! Reports average wait and current queue length.
//!
//! Usage:
//!   cog-queue-length --once
//!   cog-queue-length --interval 5

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

struct QueueTracker {
    variance_threshold: f64,
    // Track when each zone first detected presence (for wait time)
    zone_arrival: HashMap<String, u64>,
    // Wait time stats
    wait_stats: WelfordStats,
    total_served: u64,
    peak_length: u32,
    baselines: HashMap<String, WelfordStats>,
}

impl QueueTracker {
    fn new(threshold: f64) -> Self {
        Self {
            variance_threshold: threshold,
            zone_arrival: HashMap::new(),
            wait_stats: WelfordStats::new(),
            total_served: 0,
            peak_length: 0,
            baselines: HashMap::new(),
        }
    }

    fn process(&mut self, samples: &[(String, f64)], now: u64) -> QueueReport {
        // Compute per-channel variance
        let mut channel_vals: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_vals.entry(ch.clone()).or_default().push(*val);
        }

        let mut occupied_zones = Vec::new();
        for (ch, vals) in &channel_vals {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();
            let present = var > self.variance_threshold;

            if present {
                occupied_zones.push(ch.clone());
                // Record arrival if new
                self.zone_arrival.entry(ch.clone()).or_insert(now);
            } else {
                self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new).update(var);
                // If was in queue and now departed, record wait time
                if let Some(arrival) = self.zone_arrival.remove(ch) {
                    let wait = now - arrival;
                    if wait > 0 {
                        self.wait_stats.update(wait as f64);
                        self.total_served += 1;
                    }
                }
            }
        }

        let current_length = occupied_zones.len() as u32;
        if current_length > self.peak_length {
            self.peak_length = current_length;
        }

        // Compute longest current wait
        let longest_wait = self.zone_arrival.values()
            .map(|&a| now.saturating_sub(a))
            .max()
            .unwrap_or(0);

        QueueReport {
            current_length,
            occupied_positions: occupied_zones,
            avg_wait_secs: if self.wait_stats.count > 0 { self.wait_stats.mean.round() as u64 } else { 0 },
            longest_current_wait_secs: longest_wait,
            total_served: self.total_served,
            peak_length: self.peak_length,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct QueueReport {
    current_length: u32,
    occupied_positions: Vec<String>,
    avg_wait_secs: u64,
    longest_current_wait_secs: u64,
    total_served: u64,
    peak_length: u32,
    timestamp: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(report: &QueueReport) -> Result<(), String> {
    let vector = vec![
        report.current_length as f64,
        report.avg_wait_secs as f64 / 300.0, // normalize to 5min
        report.longest_current_wait_secs as f64 / 300.0,
        report.total_served as f64,
        report.peak_length as f64,
        0.0, 0.0, 0.0,
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

fn run_once(tracker: &mut QueueTracker) -> Result<QueueReport, String> {
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

    let report = tracker.process(&parsed, now_secs());

    if report.current_length > 5 {
        eprintln!("[cog-queue-length] ALERT: long queue={}, wait={}s",
            report.current_length, report.longest_current_wait_secs);
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

    eprintln!("[cog-queue-length] starting (interval={}s)", interval);
    let mut tracker = QueueTracker::new(10.0);

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-queue-length] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-queue-length] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
