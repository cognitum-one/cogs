//! Cognitum Cog: HVAC Presence
//!
//! Arrival/departure detection with debounce timers for HVAC control.
//! On arrival: signal "heat_on". On departure after 15min timeout: signal "off".
//! Tracks schedule patterns by hour-of-day.
//!
//! Usage:
//!   cog-hvac-presence --once
//!   cog-hvac-presence --interval 10

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

struct HvacController {
    variance_threshold: f64,
    departure_timeout_secs: u64,
    debounce_on: u32,
    debounce_off: u32,
    on_count: u32,
    off_count: u32,
    is_present: bool,
    hvac_state: bool,
    last_departure_time: Option<u64>,
    arrival_count: u64,
    departure_count: u64,
    // Schedule pattern: arrivals per hour (0-23)
    hourly_arrivals: [u32; 24],
    baseline: WelfordStats,
}

impl HvacController {
    fn new(threshold: f64, departure_timeout: u64) -> Self {
        Self {
            variance_threshold: threshold,
            departure_timeout_secs: departure_timeout,
            debounce_on: 3,
            debounce_off: 5,
            on_count: 0,
            off_count: 0,
            is_present: false,
            hvac_state: false,
            last_departure_time: None,
            arrival_count: 0,
            departure_count: 0,
            hourly_arrivals: [0; 24],
            baseline: WelfordStats::new(),
        }
    }

    fn update(&mut self, signal_variance: f64, now: u64) -> HvacReport {
        let raw_present = signal_variance > self.variance_threshold;
        let was_present = self.is_present;
        let hour = ((now % 86400) / 3600) as usize;

        if raw_present {
            self.on_count += 1;
            self.off_count = 0;
            if self.on_count >= self.debounce_on && !self.is_present {
                self.is_present = true;
                self.arrival_count += 1;
                self.hourly_arrivals[hour] += 1;
                self.last_departure_time = None;
            }
        } else {
            self.off_count += 1;
            self.on_count = 0;
            if self.off_count >= self.debounce_off && self.is_present {
                self.is_present = false;
                self.departure_count += 1;
                self.last_departure_time = Some(now);
            }
            self.baseline.update(signal_variance);
        }

        // HVAC logic: turn on immediately on arrival, off after timeout
        let mut command = "hold".to_string();
        if self.is_present && !self.hvac_state {
            self.hvac_state = true;
            command = "heat_on".to_string();
        } else if !self.is_present && self.hvac_state {
            if let Some(dep_time) = self.last_departure_time {
                if now - dep_time >= self.departure_timeout_secs {
                    self.hvac_state = false;
                    command = "off".to_string();
                }
            }
        }

        let event = if self.is_present && !was_present {
            "arrival".to_string()
        } else if !self.is_present && was_present {
            "departure".to_string()
        } else {
            "none".to_string()
        };

        // Find peak hours
        let peak_hour = self.hourly_arrivals.iter()
            .enumerate()
            .max_by_key(|(_, c)| *c)
            .map(|(h, _)| h as u32)
            .unwrap_or(0);

        HvacReport {
            present: self.is_present,
            hvac_state: self.hvac_state,
            command,
            event,
            signal_variance,
            arrival_count: self.arrival_count,
            departure_count: self.departure_count,
            peak_arrival_hour: peak_hour,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct HvacReport {
    present: bool,
    hvac_state: bool,
    command: String,
    event: String,
    signal_variance: f64,
    arrival_count: u64,
    departure_count: u64,
    peak_arrival_hour: u32,
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

fn store_vector(report: &HvacReport) -> Result<(), String> {
    let vector = vec![
        if report.present { 1.0 } else { 0.0 },
        if report.hvac_state { 1.0 } else { 0.0 },
        match report.command.as_str() { "heat_on" => 1.0, "off" => -1.0, _ => 0.0 },
        report.signal_variance / 100.0,
        report.arrival_count as f64,
        report.departure_count as f64,
        report.peak_arrival_hour as f64 / 24.0,
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

fn run_once(ctrl: &mut HvacController) -> Result<HvacReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let mut stats = WelfordStats::new();
    for ch in samples.iter().take(256) {
        if let Some(val) = ch.get("value").and_then(|v| v.as_f64()) {
            stats.update(val);
        }
    }
    if stats.count < 2 {
        return Err("insufficient sensor data".into());
    }

    let report = ctrl.update(stats.variance(), now_secs());

    if report.command != "hold" {
        eprintln!("[cog-hvac-presence] ALERT: HVAC command={}", report.command);
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

    eprintln!("[cog-hvac-presence] starting (interval={}s, departure_timeout=900s)", interval);
    let mut ctrl = HvacController::new(10.0, 900); // 15 min timeout

    loop {
        let start = Instant::now();
        match run_once(&mut ctrl) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-hvac-presence] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-hvac-presence] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
