//! Cognitum Cog: Perimeter Breach — Multi-Zone Monitoring
//!
//! Tracks signal patterns per channel as "zones". Detects entry direction
//! by which zone triggers first. Uses Welford baseline per zone with
//! z-score alerting.
//!
//! Usage:
//!   cog-perimeter-breach --once
//!   cog-perimeter-breach --interval 2

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
    fn std_dev(&self) -> f64 {
        if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() }
    }
    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { 0.0 } else { (value - self.mean) / sd }
    }
}

const MAX_ZONES: usize = 16;

struct ZoneState {
    baseline: WelfordStats,
    last_trigger_time: Option<Instant>,
    triggered: bool,
    zone_id: String,
}

impl ZoneState {
    fn new(zone_id: String) -> Self {
        Self {
            baseline: WelfordStats::new(),
            last_trigger_time: None,
            triggered: false,
            zone_id,
        }
    }
}

struct PerimeterMonitor {
    zones: Vec<ZoneState>,
    threshold: f64,
    learning_samples: u64,
    breach_window_ms: u64,
    recent_breaches: Vec<BreachEvent>,
}

struct BreachEvent {
    entry_zone: String,
    exit_zone: Option<String>,
    direction: String,
    confidence: f64,
    timestamp: u64,
}

impl PerimeterMonitor {
    fn new(threshold: f64) -> Self {
        Self {
            zones: Vec::new(),
            threshold,
            learning_samples: 30,
            breach_window_ms: 5000,
            recent_breaches: Vec::new(),
        }
    }

    fn get_or_create_zone(&mut self, zone_id: &str) -> usize {
        if let Some(idx) = self.zones.iter().position(|z| z.zone_id == zone_id) {
            return idx;
        }
        if self.zones.len() < MAX_ZONES {
            self.zones.push(ZoneState::new(zone_id.to_string()));
            self.zones.len() - 1
        } else {
            0 // fallback to first zone if at max
        }
    }

    fn process_sample(&mut self, zone_id: &str, value: f64) -> Option<ZoneTrigger> {
        let idx = self.get_or_create_zone(zone_id);
        let zone = &mut self.zones[idx];

        if zone.baseline.count < self.learning_samples {
            zone.baseline.update(value);
            return None;
        }

        let z = zone.baseline.z_score(value);
        if z.abs() > self.threshold {
            let was_triggered = zone.triggered;
            zone.triggered = true;
            zone.last_trigger_time = Some(Instant::now());
            if !was_triggered {
                return Some(ZoneTrigger {
                    zone_id: zone.zone_id.clone(),
                    z_score: z,
                    value,
                });
            }
        } else {
            zone.triggered = false;
            // slow adaptation
            zone.baseline.update(value);
        }
        None
    }

    fn detect_breach(&mut self, triggers: &[ZoneTrigger], now_ts: u64) -> Option<BreachEvent> {
        if triggers.is_empty() {
            return None;
        }

        // Check for multi-zone trigger within window — indicates directional breach
        let triggered_zones: Vec<&str> = self.zones.iter()
            .filter(|z| z.triggered)
            .map(|z| z.zone_id.as_str())
            .collect();

        if triggered_zones.len() >= 2 {
            // Direction: first triggered zone -> second triggered zone
            let first = &triggers[0];
            let second_zone = triggered_zones.iter()
                .find(|z| **z != first.zone_id)
                .map(|s| s.to_string());

            let direction = if let Some(ref exit) = second_zone {
                format!("{} -> {}", first.zone_id, exit)
            } else {
                format!("into {}", first.zone_id)
            };

            let max_z = triggers.iter().map(|t| t.z_score.abs()).fold(0.0_f64, f64::max);
            let confidence = (max_z / (self.threshold * 2.0)).min(1.0);

            let breach = BreachEvent {
                entry_zone: first.zone_id.clone(),
                exit_zone: second_zone,
                direction,
                confidence,
                timestamp: now_ts,
            };

            // Reset triggers after breach detected
            for zone in &mut self.zones {
                zone.triggered = false;
            }

            self.recent_breaches.push(BreachEvent {
                entry_zone: breach.entry_zone.clone(),
                exit_zone: breach.exit_zone.clone(),
                direction: breach.direction.clone(),
                confidence: breach.confidence,
                timestamp: breach.timestamp,
            });

            return Some(breach);
        }

        // Single zone trigger
        if triggers.len() == 1 {
            let t = &triggers[0];
            let confidence = (t.z_score.abs() / (self.threshold * 2.0)).min(1.0);
            if confidence > 0.6 {
                let breach = BreachEvent {
                    entry_zone: t.zone_id.clone(),
                    exit_zone: None,
                    direction: format!("into {}", t.zone_id),
                    confidence,
                    timestamp: now_ts,
                };
                return Some(breach);
            }
        }

        None
    }
}

struct ZoneTrigger {
    zone_id: String,
    z_score: f64,
    value: f64,
}

#[derive(serde::Serialize)]
struct PerimeterReport {
    status: String,
    breach_detected: bool,
    entry_zone: String,
    exit_zone: String,
    direction: String,
    confidence: f64,
    active_zones: usize,
    triggered_zones: usize,
    total_breaches: u64,
    timestamp: u64,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(report: &PerimeterReport) -> Result<(), String> {
    let vector = vec![
        if report.breach_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.triggered_zones as f64 / 16.0,
        report.active_zones as f64 / 16.0,
        report.total_breaches as f64 / 1000.0,
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2);
    let threshold = args.iter()
        .position(|a| a == "--threshold")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(3.0);

    eprintln!("[cog-perimeter-breach] starting (interval={}s, threshold={:.1})", interval, threshold);
    let mut monitor = PerimeterMonitor::new(threshold);
    let mut total_breaches: u64 = 0;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|s| s.as_array());
                if let Some(chs) = samples {
                    let mut triggers = Vec::new();
                    for ch in chs {
                        let zone_id = ch.get("channel")
                            .and_then(|c| c.as_str())
                            .unwrap_or("ch0")
                            .to_string();
                        let value = ch.get("value")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        if let Some(trigger) = monitor.process_sample(&zone_id, value) {
                            triggers.push(trigger);
                        }
                    }

                    let ts = now_ts();
                    let breach = monitor.detect_breach(&triggers, ts);
                    let breach_detected = breach.is_some();
                    if breach_detected { total_breaches += 1; }

                    let triggered_count = monitor.zones.iter().filter(|z| z.triggered).count();
                    let report = PerimeterReport {
                        status: if breach_detected { "BREACH".into() }
                            else if triggered_count > 0 { "alert".into() }
                            else { "clear".into() },
                        breach_detected,
                        entry_zone: breach.as_ref().map(|b| b.entry_zone.clone()).unwrap_or_default(),
                        exit_zone: breach.as_ref().and_then(|b| b.exit_zone.clone()).unwrap_or_default(),
                        direction: breach.as_ref().map(|b| b.direction.clone()).unwrap_or_default(),
                        confidence: breach.as_ref().map(|b| b.confidence).unwrap_or(0.0),
                        active_zones: monitor.zones.len(),
                        triggered_zones: triggered_count,
                        total_breaches,
                        timestamp: ts,
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-perimeter-breach] store error: {e}");
                    }
                    if breach_detected {
                        eprintln!("[cog-perimeter-breach] BREACH: {} (confidence={:.0}%)",
                            report.direction, report.confidence * 100.0);
                    }
                }
            }
            Err(e) => eprintln!("[cog-perimeter-breach] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
