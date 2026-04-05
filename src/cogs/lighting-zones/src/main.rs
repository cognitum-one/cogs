//! Cognitum Cog: Lighting Zones
//!
//! Tracks movement between rooms (channels). When presence detected in
//! zone N, output "lights_on" for zone N. Auto-off after configurable timeout.
//!
//! Usage:
//!   cog-lighting-zones --once
//!   cog-lighting-zones --interval 5

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

struct LightingController {
    variance_threshold: f64,
    auto_off_secs: u64,
    zone_lights: HashMap<String, bool>,
    zone_last_presence: HashMap<String, u64>,
    baselines: HashMap<String, WelfordStats>,
    transitions: u64,
    last_active_zone: Option<String>,
}

impl LightingController {
    fn new(threshold: f64, auto_off: u64) -> Self {
        Self {
            variance_threshold: threshold,
            auto_off_secs: auto_off,
            zone_lights: HashMap::new(),
            zone_last_presence: HashMap::new(),
            baselines: HashMap::new(),
            transitions: 0,
            last_active_zone: None,
        }
    }

    fn process(&mut self, samples: &[(String, f64)], now: u64) -> LightingReport {
        // Compute per-channel variance
        let mut channel_vals: HashMap<String, Vec<f64>> = HashMap::new();
        for (ch, val) in samples {
            channel_vals.entry(ch.clone()).or_default().push(*val);
        }

        let mut commands = Vec::new();
        let mut active_zones = Vec::new();

        for (ch, vals) in &channel_vals {
            let mut stats = WelfordStats::new();
            for v in vals { stats.update(*v); }
            let var = stats.variance();

            let present = var > self.variance_threshold;

            if present {
                self.zone_last_presence.insert(ch.clone(), now);
                let was_on = *self.zone_lights.get(ch).unwrap_or(&false);
                self.zone_lights.insert(ch.clone(), true);
                active_zones.push(ch.clone());

                if !was_on {
                    commands.push(LightCommand { zone: ch.clone(), action: "lights_on".into() });
                    if let Some(ref last) = self.last_active_zone {
                        if last != ch { self.transitions += 1; }
                    }
                    self.last_active_zone = Some(ch.clone());
                }
            } else {
                self.baselines.entry(ch.clone()).or_insert_with(WelfordStats::new).update(var);
            }
        }

        // Auto-off for zones with expired timeout
        let zone_keys: Vec<String> = self.zone_lights.keys().cloned().collect();
        for zone in zone_keys {
            if *self.zone_lights.get(&zone).unwrap_or(&false) {
                if let Some(last) = self.zone_last_presence.get(&zone) {
                    if now - last >= self.auto_off_secs && !active_zones.contains(&zone) {
                        self.zone_lights.insert(zone.clone(), false);
                        commands.push(LightCommand { zone, action: "lights_off".into() });
                    }
                }
            }
        }

        let lit_count = self.zone_lights.values().filter(|&&v| v).count() as u32;

        LightingReport {
            active_zones,
            lit_zones: lit_count,
            commands,
            zone_transitions: self.transitions,
            timestamp: now,
        }
    }
}

#[derive(serde::Serialize)]
struct LightCommand {
    zone: String,
    action: String,
}

#[derive(serde::Serialize)]
struct LightingReport {
    active_zones: Vec<String>,
    lit_zones: u32,
    commands: Vec<LightCommand>,
    zone_transitions: u64,
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

fn store_vector(report: &LightingReport) -> Result<(), String> {
    let vector = vec![
        report.active_zones.len() as f64,
        report.lit_zones as f64,
        report.commands.len() as f64,
        report.zone_transitions as f64,
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

fn run_once(ctrl: &mut LightingController) -> Result<LightingReport, String> {
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

    let report = ctrl.process(&parsed, now_secs());

    for cmd in &report.commands {
        eprintln!("[cog-lighting-zones] ALERT: {} -> {}", cmd.zone, cmd.action);
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

    eprintln!("[cog-lighting-zones] starting (interval={}s, auto_off=300s)", interval);
    let mut ctrl = LightingController::new(10.0, 300); // 5 min auto-off

    loop {
        let start = Instant::now();
        match run_once(&mut ctrl) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_vector(&report) {
                    eprintln!("[cog-lighting-zones] store error: {e}");
                }
            }
            Err(e) => eprintln!("[cog-lighting-zones] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
