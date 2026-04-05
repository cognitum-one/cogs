//! Cognitum Cog: Table Turnover
//!
//! Tracks table occupancy cycles per zone. Detects sit-down (presence start),
//! meal duration, departure. Computes turnover rate per hour.
//!
//! Usage:
//!   cog-table-turnover --once
//!   cog-table-turnover --interval 10

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

struct TableState {
    occupied: bool,
    sit_down_time: Option<u64>,
    debounce_on: u32,
    debounce_off: u32,
    on_count: u32,
    off_count: u32,
}

struct TurnoverTracker {
    variance_threshold: f64,
    tables: HashMap<String, TableState>,
    /// Per-table completed meal durations
    meal_durations: HashMap<String, Vec<u64>>,
    total_turnovers: u64,
    start_time: u64,
    baselines: HashMap<String, WelfordStats>,
    duration_stats: WelfordStats,
}

impl TurnoverTracker {
    fn new(threshold: f64, now: u64) -> Self {
        Self {
            variance_threshold: threshold,
            tables: HashMap::new(),
            meal_durations: HashMap::new(),
            total_turnovers: 0,
            start_time: now,
            baselines: HashMap::new(),
            duration_stats: WelfordStats::new(),
        }
    }

    fn process(&mut self, samples: &[(String, f64)], now: u64) -> TurnoverReport {
        let mut channel_vals: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_vals.entry(ch.clone()).or_default().push(*val);
        }

        let mut events = Vec::new();

        for (ch, vals) in &channel_vals {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();
            let raw_present = var > self.variance_threshold;

            let table = self.tables.entry(ch.clone()).or_insert(TableState {
                occupied: false,
                sit_down_time: None,
                debounce_on: 3,
                debounce_off: 6,
                on_count: 0,
                off_count: 0,
            });

            let was_occupied = table.occupied;

            if raw_present {
                table.on_count += 1;
                table.off_count = 0;
                if table.on_count >= table.debounce_on && !table.occupied {
                    table.occupied = true;
                    table.sit_down_time = Some(now);
                    events.push(TableEvent { table: ch.clone(), event: "sit_down".into(), duration_secs: 0 });
                }
            } else {
                table.off_count += 1;
                table.on_count = 0;
                if table.off_count >= table.debounce_off && table.occupied {
                    table.occupied = false;
                    let duration = table.sit_down_time.take().map(|s| now - s).unwrap_or(0);
                    if duration > 60 { // Only count meals > 1 min
                        self.meal_durations.entry(ch.clone()).or_default().push(duration);
                        self.total_turnovers += 1;
                        self.duration_stats.update(duration as f64);
                        events.push(TableEvent { table: ch.clone(), event: "departure".into(), duration_secs: duration });
                    }
                }
                self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new).update(var);
            }
        }

        let elapsed_hours = (now - self.start_time).max(1) as f64 / 3600.0;
        let turnover_rate = self.total_turnovers as f64 / elapsed_hours.max(0.01);

        let occupied_count = self.tables.values().filter(|t| t.occupied).count() as u32;
        let total_tables = self.tables.len() as u32;

        // Per-table stats
        let mut table_stats: Vec<TableStat> = Vec::new();
        for (table_id, durations) in &self.meal_durations {
            let avg = if durations.is_empty() { 0 } else {
                (durations.iter().sum::<u64>() / durations.len() as u64)
            };
            table_stats.push(TableStat {
                table: table_id.clone(),
                turns: durations.len() as u32,
                avg_duration_secs: avg,
                occupied: self.tables.get(table_id).map(|t| t.occupied).unwrap_or(false),
            });
        }
        table_stats.sort_by(|a, b| b.turns.cmp(&a.turns));

        TurnoverReport {
            occupied_tables: occupied_count,
            total_tables,
            total_turnovers: self.total_turnovers,
            turnover_rate_per_hour: (turnover_rate * 10.0).round() / 10.0,
            avg_meal_duration_secs: if self.duration_stats.count > 0 { self.duration_stats.mean.round() as u64 } else { 0 },
            events,
            table_stats,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct TableEvent {
    table: String,
    event: String,
    duration_secs: u64,
}

#[derive(serde::Serialize)]
struct TableStat {
    table: String,
    turns: u32,
    avg_duration_secs: u64,
    occupied: bool,
}

#[derive(serde::Serialize)]
struct TurnoverReport {
    occupied_tables: u32,
    total_tables: u32,
    total_turnovers: u64,
    turnover_rate_per_hour: f64,
    avg_meal_duration_secs: u64,
    events: Vec<TableEvent>,
    table_stats: Vec<TableStat>,
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

fn store_vector(report: &TurnoverReport) -> Result<(), String> {
    let vector = vec![
        report.occupied_tables as f64,
        report.total_tables as f64,
        report.turnover_rate_per_hour,
        report.avg_meal_duration_secs as f64 / 3600.0,
        report.total_turnovers as f64,
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

fn run_once(tracker: &mut TurnoverTracker) -> Result<TurnoverReport, String> {
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
        eprintln!("[cog-table-turnover] EVENT: {} {} ({}s)", ev.table, ev.event, ev.duration_secs);
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
        .unwrap_or(10);

    eprintln!("[cog-table-turnover] starting (interval={}s)", interval);
    let mut tracker = TurnoverTracker::new(10.0, now_secs());

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-table-turnover] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-table-turnover] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
