//! Cognitum Cog: Shelf Engagement
//!
//! Detects brief interactions near specific zones. Short dwell (5-30s)
//! near a channel = engagement event. Tracks engagement rate per zone.
//! Distinguishes pass-through (<5s), engagement (5-30s), and lingering (>30s).
//!
//! Usage:
//!   cog-shelf-engagement --once
//!   cog-shelf-engagement --interval 5

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

struct ZoneEngagement {
    pass_throughs: u64,   // <5s
    engagements: u64,      // 5-30s
    lingering: u64,        // >30s
    total_visits: u64,
}

impl ZoneEngagement {
    fn new() -> Self { Self { pass_throughs: 0, engagements: 0, lingering: 0, total_visits: 0 } }
    fn engagement_rate(&self) -> f64 {
        if self.total_visits == 0 { 0.0 } else { self.engagements as f64 / self.total_visits as f64 }
    }
}

struct ShelfTracker {
    variance_threshold: f64,
    min_engagement_secs: u64,
    max_engagement_secs: u64,
    /// Per-zone: when current presence started
    zone_presence_start: HashMap<String, u64>,
    /// Per-zone engagement stats
    zone_stats: HashMap<String, ZoneEngagement>,
    baselines: HashMap<String, WelfordStats>,
}

impl ShelfTracker {
    fn new(threshold: f64) -> Self {
        Self {
            variance_threshold: threshold,
            min_engagement_secs: 5,
            max_engagement_secs: 30,
            zone_presence_start: HashMap::new(),
            zone_stats: HashMap::new(),
            baselines: HashMap::new(),
        }
    }

    fn process(&mut self, samples: &[(String, f64)], now: u64) -> ShelfReport {
        let mut channel_vals: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_vals.entry(ch.clone()).or_default().push(*val);
        }

        let mut events = Vec::new();

        for (ch, vals) in &channel_vals {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();
            let present = var > self.variance_threshold;

            if present {
                self.zone_presence_start.entry(ch.clone()).or_insert(now);
            } else {
                self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new).update(var);

                // Classify completed interaction
                if let Some(start) = self.zone_presence_start.remove(ch) {
                    let dwell = now - start;
                    let zone_eng = self.zone_stats.entry(ch.clone()).or_insert_with(ZoneEngagement::new);
                    zone_eng.total_visits += 1;

                    let interaction_type = if dwell < self.min_engagement_secs {
                        zone_eng.pass_throughs += 1;
                        "pass_through"
                    } else if dwell <= self.max_engagement_secs {
                        zone_eng.engagements += 1;
                        "engagement"
                    } else {
                        zone_eng.lingering += 1;
                        "lingering"
                    };

                    events.push(EngagementEvent {
                        zone: ch.clone(),
                        interaction_type: interaction_type.into(),
                        dwell_secs: dwell,
                    });
                }
            }
        }

        // Build per-zone summary sorted by engagement rate
        let mut zone_summary: Vec<ZoneSummary> = self.zone_stats.iter().map(|(zone, eng)| {
            ZoneSummary {
                zone: zone.clone(),
                engagement_rate: (eng.engagement_rate() * 100.0).round(),
                total_visits: eng.total_visits,
                engagements: eng.engagements,
                pass_throughs: eng.pass_throughs,
                lingering: eng.lingering,
                active: self.zone_presence_start.contains_key(zone),
            }
        }).collect();
        zone_summary.sort_by(|a, b| b.engagement_rate.partial_cmp(&a.engagement_rate).unwrap_or(std::cmp::Ordering::Equal));

        let total_engagements: u64 = self.zone_stats.values().map(|e| e.engagements).sum();
        let total_visits: u64 = self.zone_stats.values().map(|e| e.total_visits).sum();
        let overall_rate = if total_visits > 0 { total_engagements as f64 / total_visits as f64 } else { 0.0 };

        let top_zone = zone_summary.first().map(|z| z.zone.clone()).unwrap_or_default();

        ShelfReport {
            events,
            zone_summary,
            top_engagement_zone: top_zone,
            overall_engagement_rate: (overall_rate * 100.0).round(),
            total_engagements,
            total_visits,
            active_zones: self.zone_presence_start.len() as u32,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct EngagementEvent {
    zone: String,
    interaction_type: String,
    dwell_secs: u64,
}

#[derive(serde::Serialize)]
struct ZoneSummary {
    zone: String,
    engagement_rate: f64,
    total_visits: u64,
    engagements: u64,
    pass_throughs: u64,
    lingering: u64,
    active: bool,
}

#[derive(serde::Serialize)]
struct ShelfReport {
    events: Vec<EngagementEvent>,
    zone_summary: Vec<ZoneSummary>,
    top_engagement_zone: String,
    overall_engagement_rate: f64,
    total_engagements: u64,
    total_visits: u64,
    active_zones: u32,
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

fn store_vector(report: &ShelfReport) -> Result<(), String> {
    let vector = vec![
        report.total_engagements as f64,
        report.total_visits as f64,
        report.overall_engagement_rate / 100.0,
        report.active_zones as f64,
        report.zone_summary.len() as f64,
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

fn run_once(tracker: &mut ShelfTracker) -> Result<ShelfReport, String> {
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

    for ev in &report.events {
        if ev.interaction_type == "engagement" {
            eprintln!("[cog-shelf-engagement] ENGAGEMENT: {} ({}s)", ev.zone, ev.dwell_secs);
        }
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

    eprintln!("[cog-shelf-engagement] starting (interval={}s, engagement=5-30s)", interval);
    let mut tracker = ShelfTracker::new(10.0);

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-shelf-engagement] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-shelf-engagement] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
