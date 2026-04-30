//! Cognitum Cog: Slip / Wet-Floor Zone (ADR-010)
//!
//! Pre-fall risk detector fusing motion-variance drop, splash audio,
//! and optional cautious-gait CSI score.

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
    slip_risk_high: bool,
    motion_drop_z: f64,
    splash_z: f64,
    cautious_gait_score: f64,
    session_alerts: u64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.slip_risk_high { 1.0 } else { 0.0 },
        (r.motion_drop_z.abs() / 5.0).clamp(0.0, 1.0),
        (r.splash_z / 5.0).clamp(0.0, 1.0),
        r.cautious_gait_score.min(1.0),
        (r.session_alerts as f64 / 100.0).min(1.0),
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[10, v]], "dedup": true });
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
    let motion_drop_z_thr: f64 = parse_arg(&args, "--motion-drop-z").unwrap_or(1.5);
    let splash_z_thr: f64 = parse_arg(&args, "--splash-z").unwrap_or(3.0);
    let cooldown_secs: u64 = parse_arg(&args, "--cooldown").unwrap_or(600);
    let ruview_mode = args.iter().any(|a| a == "--ruview-mode");

    eprintln!("[cog-slip-fall-zone] start (interval={interval}s, motion_z={motion_drop_z_thr}, splash_z={splash_z_thr}, cooldown={cooldown_secs}s, ruview={ruview_mode})");

    let mut motion_baseline = Welford::new();
    let mut amp_baseline = Welford::new();
    let mut amp_history: VecDeque<f64> = VecDeque::with_capacity(30);
    let mut alerts: u64 = 0;
    let mut cooldown_until: Option<Instant> = None;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;
                        let max_amp = amps.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);

                        amp_history.push_back(max_amp);
                        while amp_history.len() > 30 { amp_history.pop_front(); }

                        let z_var = motion_baseline.z(var);
                        let z_amp = amp_baseline.z(max_amp);

                        // Motion drop = baseline higher than current
                        let motion_drop_z = if z_var < 0.0 { z_var.abs() } else { 0.0 };
                        // Splash = upward spike
                        let splash_z = z_amp;

                        // Cautious-gait score: ruview only — uses gradient of amp_history
                        let mut cautious_score = 0.0;
                        if ruview_mode && amp_history.len() >= 10 {
                            let recent: f64 = amp_history.iter().rev().take(5).sum::<f64>() / 5.0;
                            let earlier: f64 = amp_history.iter().take(5).sum::<f64>() / 5.0;
                            if earlier > 1e-9 {
                                let ratio = recent / earlier;
                                cautious_score = if ratio < 0.7 { (0.7 - ratio) / 0.3 } else { 0.0 };
                            }
                        }

                        let now = Instant::now();
                        let in_cooldown = cooldown_until.map(|t| now < t).unwrap_or(false);
                        if !in_cooldown { cooldown_until = None; }

                        // Update baselines on quiet frames only
                        if motion_drop_z < motion_drop_z_thr * 0.5 && splash_z < splash_z_thr * 0.5 {
                            motion_baseline.update(var);
                            amp_baseline.update(max_amp);
                        }

                        // Risk fusion — at least motion-drop AND (splash OR cautious gait)
                        let motion_signal = motion_drop_z >= motion_drop_z_thr;
                        let splash_signal = splash_z >= splash_z_thr;
                        let cautious_signal = cautious_score >= 0.5;
                        let risk = motion_signal && (splash_signal || cautious_signal) && !in_cooldown;

                        let mut fired = false;
                        let status = if in_cooldown { "cooldown" }
                                     else if risk {
                                         alerts += 1;
                                         cooldown_until = Some(now + Duration::from_secs(cooldown_secs));
                                         fired = true;
                                         "SLIP_RISK_HIGH"
                                     } else if splash_signal { "SPLASH" }
                                     else if motion_signal || cautious_signal { "cautious" }
                                     else { "normal" };

                        let r = Report {
                            status: status.into(),
                            slip_risk_high: fired,
                            motion_drop_z,
                            splash_z,
                            cautious_gait_score: cautious_score,
                            session_alerts: alerts,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-slip-fall-zone] store error: {e}"); }
                        if fired { eprintln!("[cog-slip-fall-zone] ALERT: slip risk high"); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-slip-fall-zone] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
