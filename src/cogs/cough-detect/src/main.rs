//! Cognitum Cog: Cough Detection
//!
//! ADR-003. Transient + spectral + cluster detector.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

struct Welford { count: u64, mean: f64, m2: f64 }
impl Welford {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, v: f64) {
        self.count += 1;
        let d = v - self.mean;
        self.mean += d / self.count as f64;
        self.m2 += d * (v - self.mean);
    }
    fn std_dev(&self) -> f64 { if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() } }
    fn z(&self, v: f64) -> f64 { let s = self.std_dev(); if s < 1e-10 { 0.0 } else { (v - self.mean) / s } }
}

struct CoughDetector {
    amp_baseline: Welford,
    transient_z: f64,
    cluster_window_secs: u64,
    alert_count: u32,
    events: VecDeque<Instant>,
    total: u64,
    last_z: f64,
    last_ratio: f64,
    cooldown_until: Option<Instant>,
}

impl CoughDetector {
    fn new(transient_z: f64, cluster_window_secs: u64, alert_count: u32) -> Self {
        Self { amp_baseline: Welford::new(), transient_z, cluster_window_secs, alert_count,
               events: VecDeque::new(), total: 0, last_z: 0.0, last_ratio: 0.0, cooldown_until: None }
    }

    fn update(&mut self, mean_amp: f64, low_band: f64, high_band: f64) -> Report {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(self.cluster_window_secs);
        while self.events.front().map(|&t| t < cutoff).unwrap_or(false) { self.events.pop_front(); }

        let z = self.amp_baseline.z(mean_amp);
        self.last_z = z;
        let ratio = if low_band.abs() > 1e-9 { high_band.abs() / low_band.abs() } else { 0.0 };
        self.last_ratio = ratio;

        let in_cooldown = self.cooldown_until.map(|t| now < t).unwrap_or(false);
        let triggered = !in_cooldown && z > self.transient_z && ratio > 1.5;

        if triggered {
            self.events.push_back(now);
            self.total += 1;
            self.cooldown_until = Some(now + Duration::from_millis(400)); // suppress 400ms tail
        }

        // Always update baseline (slowly, except during firing)
        if !triggered { self.amp_baseline.update(mean_amp); }

        let n = self.events.len() as u32;
        let status = if triggered && n >= self.alert_count { "cluster_alert" }
                     else if triggered { "cough" }
                     else if n >= self.alert_count { "burst" }
                     else { "quiet" };

        Report {
            status: status.into(),
            cough_detected: triggered,
            events_30s: n,
            events_total: self.total,
            transient_z: z,
            spectral_ratio: ratio,
            timestamp: now_ts(),
        }
    }
}

#[derive(serde::Serialize)]
struct Report {
    status: String,
    cough_detected: bool,
    events_30s: u32,
    events_total: u64,
    transient_z: f64,
    spectral_ratio: f64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store_to_seed(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.cough_detected { 1.0 } else { 0.0 },
        (r.events_30s as f64 / 10.0).min(1.0),
        (r.transient_z / 10.0).clamp(0.0, 1.0),
        (r.spectral_ratio / 5.0).min(1.0),
        (r.events_total as f64 / 1000.0).min(1.0),
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[3, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut c = std::net::TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new(); c.read_to_end(&mut resp).ok();
    Ok(())
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(1);
    let transient_z: f64 = parse_arg(&args, "--transient-z").unwrap_or(3.0);
    let cluster_window: u64 = parse_arg(&args, "--cluster-window").unwrap_or(30);
    let alert_count: u32 = parse_arg(&args, "--alert-count").unwrap_or(3);

    eprintln!("[cog-cough-detect] start (interval={interval}s, z={transient_z}, window={cluster_window}s, alert={alert_count})");
    let mut det = CoughDetector::new(transient_z, cluster_window, alert_count);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if amps.len() >= 4 {
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        // Split available channels in half: low band (first half) / high band (second half).
                        let mid = amps.len() / 2;
                        let low: f64 = amps[..mid].iter().map(|v| v.abs()).sum::<f64>() / mid as f64;
                        let hi_n = amps.len() - mid;
                        let high: f64 = amps[mid..].iter().map(|v| v.abs()).sum::<f64>() / hi_n as f64;
                        let r = det.update(mean, low, high);
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store_to_seed(&r) { eprintln!("[cog-cough-detect] store error: {e}"); }
                        if r.status == "cluster_alert" {
                            eprintln!("[cog-cough-detect] ALERT: cluster of {} cough events in {}s window", r.events_30s, cluster_window);
                        }
                    }
                }
            }
            Err(e) => eprintln!("[cog-cough-detect] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
