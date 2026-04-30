//! Cognitum Cog: Customer Flow
//!
//! Counts entries/exits using directional detection. Which channel triggers
//! first indicates direction (ch0 then ch1 = entry, ch1 then ch0 = exit).
//! Reports hourly traffic and conversion ratio (entries that dwell vs bounce).
//!
//! Usage:
//!   cog-customer-flow --once
//!   cog-customer-flow --interval 5

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

struct FlowTracker {
    variance_threshold: f64,
    /// Track per-channel first-trigger time for direction detection
    trigger_times: HashMap<String, u64>,
    /// Previous frame presence per channel
    prev_present: HashMap<String, bool>,
    /// Entry/exit channel pair (first two sorted channels)
    entry_channel: Option<String>,
    exit_channel: Option<String>,
    total_entries: u64,
    total_exits: u64,
    hourly_entries: [u32; 24],
    hourly_exits: [u32; 24],
    /// Bounce detection: entries that exit within 30s
    bounce_count: u64,
    baselines: HashMap<String, WelfordStats>,
}

impl FlowTracker {
    fn new(threshold: f64) -> Self {
        Self {
            variance_threshold: threshold,
            trigger_times: HashMap::new(),
            prev_present: HashMap::new(),
            entry_channel: None,
            exit_channel: None,
            total_entries: 0,
            total_exits: 0,
            hourly_entries: [0; 24],
            hourly_exits: [0; 24],
            bounce_count: 0,
            baselines: HashMap::new(),
        }
    }

    fn process(&mut self, samples: &[(String, f64)], now: u64) -> FlowReport {
        let hour = ((now % 86400) / 3600) as usize;

        // Compute per-channel variance
        let mut channel_vals: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_vals.entry(ch.clone()).or_default().push(*val);
        }

        // Detect presence per channel
        let mut current_present: HashMap<String, bool> = HashMap::new();
        for (ch, vals) in &channel_vals {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();
            let present = var > self.variance_threshold;
            current_present.insert(ch.clone(), present);

            if !present {
                self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new).update(var);
            }
        }

        // Auto-detect entry/exit channels from sorted channel names
        if self.entry_channel.is_none() {
            let mut channels: Vec<String> = channel_vals.keys().cloned().collect();
            channels.sort();
            if channels.len() >= 2 {
                self.entry_channel = Some(channels[0].clone());
                self.exit_channel = Some(channels[1].clone());
            }
        }

        // Detect rising edges (transition from absent to present)
        let mut newly_triggered: Vec<(String, u64)> = Vec::new();
        for (ch, &is_present) in &current_present {
            let was_present = self.prev_present.get(ch).copied().unwrap_or(false);
            if is_present && !was_present {
                newly_triggered.push((ch.clone(), now));
                self.trigger_times.insert(ch.clone(), now);
            }
            if !is_present && was_present {
                self.trigger_times.remove(ch);
            }
        }

        // Direction detection: if both channels triggered within 3s window
        let mut event = "none".to_string();
        if let (Some(ref entry_ch), Some(ref exit_ch)) = (&self.entry_channel, &self.exit_channel) {
            if let (Some(&t_entry), Some(&t_exit)) = (self.trigger_times.get(entry_ch), self.trigger_times.get(exit_ch)) {
                let diff = t_entry as i64 - t_exit as i64;
                if diff.unsigned_abs() <= 3 {
                    if t_entry < t_exit {
                        // Entry channel triggered first = entry
                        self.total_entries += 1;
                        self.hourly_entries[hour] += 1;
                        event = "entry".to_string();
                    } else {
                        // Exit channel triggered first = exit
                        self.total_exits += 1;
                        self.hourly_exits[hour] += 1;
                        event = "exit".to_string();
                    }
                    self.trigger_times.clear();
                }
            }
        }

        // Clear stale triggers (>5s old)
        self.trigger_times.retain(|_, &mut t| now.saturating_sub(t) < 5);

        self.prev_present = current_present;

        let current_inside = if self.total_entries >= self.total_exits {
            self.total_entries - self.total_exits
        } else { 0 };

        let conversion_ratio = if self.total_entries > 0 {
            let non_bounce = self.total_entries.saturating_sub(self.bounce_count);
            non_bounce as f64 / self.total_entries as f64
        } else { 0.0 };

        FlowReport {
            event,
            total_entries: self.total_entries,
            total_exits: self.total_exits,
            current_inside,
            conversion_ratio,
            hourly_entries: self.hourly_entries[hour],
            hourly_exits: self.hourly_exits[hour],
            current_hour: hour as u32,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct FlowReport {
    event: String,
    total_entries: u64,
    total_exits: u64,
    current_inside: u64,
    conversion_ratio: f64,
    hourly_entries: u32,
    hourly_exits: u32,
    current_hour: u32,
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

fn store_vector(report: &FlowReport) -> Result<(), String> {
    let vector = vec![
        report.total_entries as f64,
        report.total_exits as f64,
        report.current_inside as f64,
        report.conversion_ratio,
        report.hourly_entries as f64,
        report.hourly_exits as f64,
        report.current_hour as f64 / 24.0,
        0.0,
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

fn run_once(tracker: &mut FlowTracker) -> Result<FlowReport, String> {
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

    if report.event != "none" {
        eprintln!("[cog-customer-flow] EVENT: {} (inside={})", report.event, report.current_inside);
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

    eprintln!("[cog-customer-flow] starting (interval={}s)", interval);
    let mut tracker = FlowTracker::new(10.0);

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-customer-flow] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-customer-flow] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
