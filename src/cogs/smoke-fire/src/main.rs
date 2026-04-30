//! Cognitum Cog: Smoke / Fire Detection (ADR-012)
//!
//! Multi-signal fusion: acoustic crackle + thermal-drift CSI proxy +
//! optional ruview plume signature. Fires when 2-of-3 signals exceed
//! threshold (FIRE_LIKELY) or 3-of-3 (FIRE_CONFIRMED).

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

#[derive(serde::Serialize)]
struct Report {
    status: String,
    fire_likely: bool,
    fire_confirmed: bool,
    crackle_z: f64,
    thermal_drift_z: f64,
    ruview_plume_score: f64,
    signals_active: u32,
    total_alerts: u64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.fire_confirmed { 1.0 } else if r.fire_likely { 0.6 } else { 0.0 },
        (r.crackle_z / 5.0).clamp(0.0, 1.0),
        (r.thermal_drift_z.abs() / 5.0).clamp(0.0, 1.0),
        r.ruview_plume_score.min(1.0),
        (r.signals_active as f64 / 3.0).min(1.0),
        (r.total_alerts as f64 / 100.0).min(1.0),
        0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[12, v]], "dedup": true });
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
    let crackle_z_thr: f64 = parse_arg(&args, "--crackle-z").unwrap_or(2.5);
    let thermal_z_thr: f64 = parse_arg(&args, "--thermal-drift-z").unwrap_or(1.5);
    let cooldown_secs: u64 = parse_arg(&args, "--cooldown").unwrap_or(60);
    let ruview_mode = args.iter().any(|a| a == "--ruview-mode");

    eprintln!("[cog-smoke-fire] start (interval={interval}s, crackle_z={crackle_z_thr}, thermal_z={thermal_z_thr}, ruview={ruview_mode})");

    let mut crackle_baseline = Welford::new();
    let mut var_baseline = Welford::new();
    let mut var_history: VecDeque<f64> = VecDeque::with_capacity(60);
    let mut total_alerts: u64 = 0;
    let mut cooldown_until: Option<Instant> = None;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        // Crackle proxy: fast variance over recent absolute amplitudes
                        let mid = (amps.len() / 2).max(1);
                        let high_band: f64 = amps[mid..].iter().map(|v| v.abs()).sum::<f64>() / (amps.len() - mid) as f64;
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;

                        var_history.push_back(var);
                        while var_history.len() > 60 { var_history.pop_front(); }

                        let z_crackle = crackle_baseline.z(high_band);
                        // Thermal drift = sustained variance shift from baseline (negative or positive)
                        let z_thermal = var_baseline.z(var);

                        // RuView plume score: ratio of last-10-seconds variance vs first-10-seconds variance
                        let mut plume_score = 0.0;
                        if ruview_mode && var_history.len() >= 30 {
                            let early: f64 = var_history.iter().take(10).sum::<f64>() / 10.0;
                            let recent: f64 = var_history.iter().rev().take(10).sum::<f64>() / 10.0;
                            if early > 1e-9 {
                                let ratio = recent / early;
                                plume_score = (ratio - 1.0).clamp(0.0, 2.0) / 2.0;
                            }
                        }

                        let signal_crackle = z_crackle > crackle_z_thr;
                        let signal_thermal = z_thermal.abs() > thermal_z_thr;
                        let signal_plume = ruview_mode && plume_score > 0.3;
                        let n_signals = (signal_crackle as u32) + (signal_thermal as u32) + (signal_plume as u32);

                        // Update baselines on quiet frames
                        if n_signals == 0 {
                            crackle_baseline.update(high_band);
                            var_baseline.update(var);
                        }

                        let now = Instant::now();
                        let in_cooldown = cooldown_until.map(|t| now < t).unwrap_or(false);
                        if !in_cooldown { cooldown_until = None; }

                        let mut fire_likely = false;
                        let mut fire_confirmed = false;
                        let status = if in_cooldown { "cooldown" }
                                     else if n_signals >= 3 {
                                         total_alerts += 1;
                                         cooldown_until = Some(now + Duration::from_secs(cooldown_secs));
                                         fire_confirmed = true;
                                         fire_likely = true;
                                         "FIRE_CONFIRMED"
                                     } else if n_signals >= 2 {
                                         total_alerts += 1;
                                         cooldown_until = Some(now + Duration::from_secs(cooldown_secs));
                                         fire_likely = true;
                                         "FIRE_LIKELY"
                                     } else if n_signals == 1 { "monitoring" }
                                     else { "quiet" };

                        let r = Report {
                            status: status.into(),
                            fire_likely, fire_confirmed,
                            crackle_z: z_crackle,
                            thermal_drift_z: z_thermal,
                            ruview_plume_score: plume_score,
                            signals_active: n_signals,
                            total_alerts,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-smoke-fire] store error: {e}"); }
                        if fire_confirmed { eprintln!("[cog-smoke-fire] ALERT: fire confirmed (3/3 signals)"); }
                        else if fire_likely { eprintln!("[cog-smoke-fire] WARNING: fire likely ({}/3 signals)", n_signals); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-smoke-fire] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
