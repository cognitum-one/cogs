//! Cognitum Cog: Dwell Heatmap
//!
//! Tracks how long presence persists per zone (channel). Builds dwell time
//! histogram with rolling 1-hour window. Outputs top zones by dwell time.
//!
//! Usage:
//!   cog-dwell-heatmap --once
//!   cog-dwell-heatmap --interval 5

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

/// Rolling window entry: (timestamp, dwell_secs)
struct DwellEntry {
    timestamp: u64,
    dwell_secs: u64,
}

struct DwellTracker {
    variance_threshold: f64,
    window_secs: u64,
    // Per-zone: when current presence started
    zone_presence_start: HashMap<String, u64>,
    // Per-zone: completed dwell events within rolling window
    zone_dwells: HashMap<String, Vec<DwellEntry>>,
    // Per-zone: total accumulated dwell seconds
    zone_total_dwell: HashMap<String, u64>,
    baselines: HashMap<String, WelfordStats>,
}

impl DwellTracker {
    fn new(threshold: f64, window: u64) -> Self {
        Self {
            variance_threshold: threshold,
            window_secs: window,
            zone_presence_start: HashMap::new(),
            zone_dwells: HashMap::new(),
            zone_total_dwell: HashMap::new(),
            baselines: HashMap::new(),
        }
    }

    fn process(&mut self, samples: &[(String, f64)], now: u64) -> DwellReport {
        let mut channel_vals: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_vals.entry(ch.clone()).or_default().push(*val);
        }

        for (ch, vals) in &channel_vals {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();
            let present = var > self.variance_threshold;

            if present {
                self.zone_presence_start.entry(ch.clone()).or_insert(now);
            } else {
                self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new).update(var);
                // Record completed dwell
                if let Some(start) = self.zone_presence_start.remove(ch) {
                    let dwell = now - start;
                    if dwell > 0 {
                        self.zone_dwells.entry(ch.clone()).or_default()
                            .push(DwellEntry { timestamp: now, dwell_secs: dwell });
                        *self.zone_total_dwell.entry(ch.clone()).or_insert(0) += dwell;
                    }
                }
            }
        }

        // Prune entries outside rolling window
        let cutoff = now.saturating_sub(self.window_secs);
        for dwells in self.zone_dwells.values_mut() {
            dwells.retain(|e| e.timestamp >= cutoff);
        }

        // Build heatmap: sum dwell time per zone within window
        let mut zone_heatmap: Vec<ZoneHeat> = Vec::new();
        for (zone, dwells) in &self.zone_dwells {
            let window_dwell: u64 = dwells.iter().map(|e| e.dwell_secs).sum();
            // Add current ongoing dwell
            let ongoing = self.zone_presence_start.get(zone)
                .map(|&start| now.saturating_sub(start))
                .unwrap_or(0);
            let total = window_dwell + ongoing;
            if total > 0 {
                zone_heatmap.push(ZoneHeat {
                    zone: zone.clone(),
                    dwell_secs: total,
                    events: dwells.len() as u32,
                    active: self.zone_presence_start.contains_key(zone),
                });
            }
        }

        // Also add zones with ongoing presence but no completed dwells yet
        for (zone, &start) in &self.zone_presence_start {
            if !zone_heatmap.iter().any(|z| z.zone == *zone) {
                zone_heatmap.push(ZoneHeat {
                    zone: zone.clone(),
                    dwell_secs: now.saturating_sub(start),
                    events: 0,
                    active: true,
                });
            }
        }

        // Sort by dwell time descending
        zone_heatmap.sort_by(|a, b| b.dwell_secs.cmp(&a.dwell_secs));

        let top_zone = zone_heatmap.first().map(|z| z.zone.clone()).unwrap_or_default();
        let total_dwell: u64 = zone_heatmap.iter().map(|z| z.dwell_secs).sum();

        DwellReport {
            zone_heatmap,
            top_zone,
            total_dwell_secs: total_dwell,
            active_zones: self.zone_presence_start.len() as u32,
            window_secs: self.window_secs,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct ZoneHeat {
    zone: String,
    dwell_secs: u64,
    events: u32,
    active: bool,
}

#[derive(serde::Serialize)]
struct DwellReport {
    zone_heatmap: Vec<ZoneHeat>,
    top_zone: String,
    total_dwell_secs: u64,
    active_zones: u32,
    window_secs: u64,
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
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON in response")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(report: &DwellReport) -> Result<(), String> {
    let top_dwell = report.zone_heatmap.first().map(|z| z.dwell_secs as f64).unwrap_or(0.0);
    let vector = vec![
        report.active_zones as f64,
        report.total_dwell_secs as f64 / 3600.0,
        top_dwell / 3600.0,
        report.zone_heatmap.len() as f64,
        0.0, 0.0, 0.0, 0.0,
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

fn run_once(tracker: &mut DwellTracker) -> Result<DwellReport, String> {
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

    eprintln!("[cog-dwell-heatmap] starting (interval={}s, window=3600s)", interval);
    let mut tracker = DwellTracker::new(10.0, 3600); // 1-hour window

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-dwell-heatmap] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-dwell-heatmap] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
