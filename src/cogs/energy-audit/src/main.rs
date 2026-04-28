//! Cognitum Cog: Energy Audit
//!
//! Learns daily usage patterns using hourly Welford statistics. Detects
//! wasted energy when presence=0 but scheduled HVAC/lights would be on.
//! Outputs savings estimate in hours-of-waste per day.
//!
//! Usage:
//!   cog-energy-audit --once
//!   cog-energy-audit --interval 60

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
    fn mean(&self) -> f64 { self.mean }
}

struct EnergyAuditor {
    variance_threshold: f64,
    // Hourly presence stats (0-23)
    hourly_presence: [WelfordStats; 24],
    hourly_variance: [WelfordStats; 24],
    // Track waste: hours where typical schedule says "on" but presence=0
    waste_readings: u64,
    total_readings: u64,
    // Typical business hours (default 8-18)
    schedule_start: usize,
    schedule_end: usize,
}

impl EnergyAuditor {
    fn new(threshold: f64) -> Self {
        Self {
            variance_threshold: threshold,
            hourly_presence: std::array::from_fn(|_| WelfordStats::new()),
            hourly_variance: std::array::from_fn(|_| WelfordStats::new()),
            waste_readings: 0,
            total_readings: 0,
            schedule_start: 8,
            schedule_end: 18,
        }
    }

    fn update(&mut self, signal_variance: f64, now: u64) -> EnergyReport {
        let hour = ((now % 86400) / 3600) as usize;
        let is_present = signal_variance > self.variance_threshold;

        // Update hourly stats
        self.hourly_presence[hour].update(if is_present { 1.0 } else { 0.0 });
        self.hourly_variance[hour].update(signal_variance);
        self.total_readings += 1;

        // Detect waste: during scheduled hours but no presence
        let in_schedule = hour >= self.schedule_start && hour < self.schedule_end;
        let is_waste = in_schedule && !is_present;
        if is_waste {
            self.waste_readings += 1;
        }

        // Compute hourly profile
        let mut hourly_profile = Vec::new();
        for h in 0..24 {
            let presence_rate = self.hourly_presence[h].mean();
            let avg_variance = self.hourly_variance[h].mean();
            if self.hourly_presence[h].count > 0 {
                hourly_profile.push(HourlyBucket {
                    hour: h as u32,
                    presence_rate,
                    avg_variance,
                    samples: self.hourly_presence[h].count,
                });
            }
        }

        // Estimate waste as fraction of scheduled hours
        let scheduled_readings = self.hourly_presence[self.schedule_start..self.schedule_end]
            .iter().map(|s| s.count).sum::<u64>();
        let waste_pct = if scheduled_readings > 0 {
            self.waste_readings as f64 / scheduled_readings as f64
        } else { 0.0 };

        let scheduled_hours = (self.schedule_end - self.schedule_start) as f64;
        let estimated_waste_hours_per_day = waste_pct * scheduled_hours;

        // Find off-peak hours with high presence (after-hours usage)
        let after_hours_presence: f64 = (0..24)
            .filter(|&h| h < self.schedule_start || h >= self.schedule_end)
            .map(|h| self.hourly_presence[h].mean())
            .sum::<f64>();

        EnergyReport {
            current_present: is_present,
            current_hour: hour as u32,
            in_schedule,
            is_waste,
            waste_pct: (waste_pct * 100.0).round(),
            estimated_waste_hours_per_day: (estimated_waste_hours_per_day * 10.0).round() / 10.0,
            after_hours_activity: (after_hours_presence * 100.0).round(),
            hourly_profile,
            total_readings: self.total_readings,
            signal_variance,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct HourlyBucket {
    hour: u32,
    presence_rate: f64,
    avg_variance: f64,
    samples: u64,
}

#[derive(serde::Serialize)]
struct EnergyReport {
    current_present: bool,
    current_hour: u32,
    in_schedule: bool,
    is_waste: bool,
    waste_pct: f64,
    estimated_waste_hours_per_day: f64,
    after_hours_activity: f64,
    hourly_profile: Vec<HourlyBucket>,
    total_readings: u64,
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
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(report: &EnergyReport) -> Result<(), String> {
    let vector = vec![
        if report.current_present { 1.0 } else { 0.0 },
        report.waste_pct / 100.0,
        report.estimated_waste_hours_per_day / 24.0,
        report.after_hours_activity / 100.0,
        report.current_hour as f64 / 24.0,
        if report.is_waste { 1.0 } else { 0.0 },
        report.signal_variance / 100.0,
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

fn run_once(auditor: &mut EnergyAuditor) -> Result<EnergyReport, String> {
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

    let report = auditor.update(stats.variance(), now_secs());

    if report.is_waste {
        eprintln!("[cog-energy-audit] ALERT: energy waste detected (no presence during scheduled hours)");
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
        .unwrap_or(60);

    eprintln!("[cog-energy-audit] starting (interval={}s, schedule=08:00-18:00)", interval);
    let mut auditor = EnergyAuditor::new(10.0);

    loop {
        let start = Instant::now();
        match run_once(&mut auditor) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-energy-audit] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-energy-audit] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
