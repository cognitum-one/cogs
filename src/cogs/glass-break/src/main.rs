//! Cognitum Cog: Glass-Break Detection (ADR-006)
//!
//! Two-phase bang + shatter detector. Bang = variance spike + high-band
//! z > bang_z. Shatter = high-band sustained envelope within window.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

struct Welford { count: u64, mean: f64, m2: f64 }
impl Welford {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, v: f64) { self.count += 1; let d = v - self.mean; self.mean += d / self.count as f64; self.m2 += d * (v - self.mean); }
    fn std_dev(&self) -> f64 { if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() } }
    fn z(&self, v: f64) -> f64 { let s = self.std_dev(); if s < 1e-10 { 0.0 } else { (v - self.mean) / s } }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase { Quiet, AwaitingShatter { until: Instant, recent_high: f64 }, Cooldown { until: Instant } }

#[derive(serde::Serialize)]
struct Report { status: String, glass_break_detected: bool, bang_z: f64, shatter_score: f64, total_breaks: u64, timestamp: u64 }

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.glass_break_detected { 1.0 } else { 0.0 },
        (r.bang_z / 10.0).clamp(0.0, 1.0),
        r.shatter_score.min(1.0),
        (r.total_breaks as f64 / 100.0).min(1.0),
        0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[6, v]], "dedup": true });
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
    let bang_z: f64 = parse_arg(&args, "--bang-z").unwrap_or(5.0);
    let shatter_window_ms: u64 = parse_arg(&args, "--shatter-window-ms").unwrap_or(500);
    let cooldown_secs: u64 = parse_arg(&args, "--cooldown").unwrap_or(30);

    eprintln!("[cog-glass-break] start (interval={interval}s, bang_z={bang_z}, shatter_window={shatter_window_ms}ms)");

    let mut high_baseline = Welford::new();
    let mut var_baseline = Welford::new();
    let mut high_history: VecDeque<f64> = VecDeque::with_capacity(20);
    let mut phase = Phase::Quiet;
    let mut total: u64 = 0;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if amps.len() >= 2 {
                        let mid = (amps.len() / 2).max(1);
                        let high: f64 = amps[mid..].iter().map(|v| v.abs()).sum::<f64>() / (amps.len() - mid) as f64;
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;

                        let z_high = high_baseline.z(high);
                        let z_var = var_baseline.z(var);

                        // History tracking
                        high_history.push_back(high);
                        while high_history.len() > 10 { high_history.pop_front(); }

                        let now = Instant::now();
                        let mut fired = false;
                        let mut shatter_score = 0.0;

                        phase = match phase {
                            Phase::Cooldown { until } if now < until => Phase::Cooldown { until },
                            Phase::Cooldown { .. } => Phase::Quiet,
                            Phase::Quiet => {
                                if z_high > bang_z && z_var > bang_z * 0.5 {
                                    Phase::AwaitingShatter {
                                        until: now + Duration::from_millis(shatter_window_ms),
                                        recent_high: high,
                                    }
                                } else {
                                    high_baseline.update(high);
                                    var_baseline.update(var);
                                    Phase::Quiet
                                }
                            }
                            Phase::AwaitingShatter { until, recent_high } => {
                                if now > until {
                                    Phase::Quiet
                                } else {
                                    // Shatter = sustained high energy with declining envelope
                                    let still_high = z_high > bang_z * 0.4;
                                    let declining = high < recent_high * 1.05;
                                    if still_high && declining {
                                        shatter_score = (z_high / bang_z).min(1.0) * 0.7 + (if declining { 0.3 } else { 0.0 });
                                        if shatter_score > 0.6 {
                                            total += 1;
                                            fired = true;
                                            Phase::Cooldown { until: now + Duration::from_secs(cooldown_secs) }
                                        } else {
                                            Phase::AwaitingShatter { until, recent_high: high.max(recent_high) }
                                        }
                                    } else if still_high {
                                        Phase::AwaitingShatter { until, recent_high: high.max(recent_high) }
                                    } else {
                                        Phase::Quiet
                                    }
                                }
                            }
                        };

                        let status = match phase {
                            Phase::Quiet if fired => "GLASS_BREAK",
                            Phase::Cooldown { .. } if fired => "GLASS_BREAK",
                            Phase::Cooldown { .. } => "cooldown",
                            Phase::AwaitingShatter { .. } => "shatter",
                            Phase::Quiet if z_high > bang_z => "bang",
                            Phase::Quiet => "quiet",
                        };

                        let r = Report {
                            status: status.into(),
                            glass_break_detected: fired,
                            bang_z: z_high,
                            shatter_score,
                            total_breaks: total,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-glass-break] store error: {e}"); }
                        if fired { eprintln!("[cog-glass-break] ALERT: glass break detected (z={:.1}, shatter={:.2})", z_high, shatter_score); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-glass-break] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
