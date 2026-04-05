//! Cognitum Cog: Meeting Room
//!
//! Binary occupied/free status with confidence. Debounced presence +
//! time tracking. Reports meeting duration and utilization percentage.
//!
//! Usage:
//!   cog-meeting-room --once
//!   cog-meeting-room --interval 10

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

struct MeetingTracker {
    variance_threshold: f64,
    debounce_on: u32,
    debounce_off: u32,
    on_count: u32,
    off_count: u32,
    is_occupied: bool,
    meeting_start: Option<u64>,
    current_duration_secs: u64,
    total_occupied_secs: u64,
    total_tracked_secs: u64,
    meeting_count: u64,
    last_meeting_duration: u64,
    baseline: WelfordStats,
    start_time: u64,
}

impl MeetingTracker {
    fn new(threshold: f64, now: u64) -> Self {
        Self {
            variance_threshold: threshold,
            debounce_on: 3,
            debounce_off: 8, // Higher debounce-off for meeting rooms (people shift in chairs)
            on_count: 0,
            off_count: 0,
            is_occupied: false,
            meeting_start: None,
            current_duration_secs: 0,
            total_occupied_secs: 0,
            total_tracked_secs: 0,
            meeting_count: 0,
            last_meeting_duration: 0,
            baseline: WelfordStats::new(),
            start_time: now,
        }
    }

    fn update(&mut self, signal_variance: f64, now: u64) -> MeetingReport {
        self.total_tracked_secs = now - self.start_time;
        let raw_present = signal_variance > self.variance_threshold;
        let was_occupied = self.is_occupied;

        if raw_present {
            self.on_count += 1;
            self.off_count = 0;
            if self.on_count >= self.debounce_on && !self.is_occupied {
                self.is_occupied = true;
                self.meeting_start = Some(now);
                self.meeting_count += 1;
            }
        } else {
            self.off_count += 1;
            self.on_count = 0;
            if self.off_count >= self.debounce_off && self.is_occupied {
                self.is_occupied = false;
                if let Some(start) = self.meeting_start.take() {
                    let dur = now - start;
                    self.last_meeting_duration = dur;
                    self.total_occupied_secs += dur;
                }
            }
            self.baseline.update(signal_variance);
        }

        // Track current meeting duration
        self.current_duration_secs = if self.is_occupied {
            self.meeting_start.map(|s| now - s).unwrap_or(0)
        } else { 0 };

        let baseline_var = self.baseline.variance().max(1e-10);
        let ratio = signal_variance / baseline_var;
        let confidence = if self.is_occupied {
            (ratio - 1.0).clamp(0.0, 1.0)
        } else {
            (1.0 - ratio.min(1.0)).max(0.0)
        };

        let utilization = if self.total_tracked_secs > 0 {
            let active = self.total_occupied_secs + self.current_duration_secs;
            (active as f64 / self.total_tracked_secs as f64).min(1.0)
        } else { 0.0 };

        let event = if self.is_occupied && !was_occupied {
            "meeting_start".to_string()
        } else if !self.is_occupied && was_occupied {
            "meeting_end".to_string()
        } else {
            "none".to_string()
        };

        MeetingReport {
            occupied: self.is_occupied,
            confidence,
            event,
            current_duration_secs: self.current_duration_secs,
            last_meeting_duration_secs: self.last_meeting_duration,
            meeting_count: self.meeting_count,
            utilization_pct: (utilization * 100.0).round(),
            signal_variance,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct MeetingReport {
    occupied: bool,
    confidence: f64,
    event: String,
    current_duration_secs: u64,
    last_meeting_duration_secs: u64,
    meeting_count: u64,
    utilization_pct: f64,
    signal_variance: f64,
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

fn store_vector(report: &MeetingReport) -> Result<(), String> {
    let vector = vec![
        if report.occupied { 1.0 } else { 0.0 },
        report.confidence,
        report.current_duration_secs as f64 / 3600.0,
        report.utilization_pct / 100.0,
        report.meeting_count as f64,
        report.signal_variance / 100.0,
        0.0, 0.0,
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

fn run_once(tracker: &mut MeetingTracker) -> Result<MeetingReport, String> {
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

    let report = tracker.update(stats.variance(), now_secs());

    if report.event != "none" {
        eprintln!("[cog-meeting-room] ALERT: {}", report.event);
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

    eprintln!("[cog-meeting-room] starting (interval={}s)", interval);
    let mut tracker = MeetingTracker::new(10.0, now_secs());

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-meeting-room] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-meeting-room] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
