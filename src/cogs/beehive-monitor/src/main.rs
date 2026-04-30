//! Cognitum Cog: Beehive Monitor (ADR-014)

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
    queen_loss_likely: bool,
    swarming_likely: bool,
    robbing_likely: bool,
    hum_energy: f64,
    chaos_z: f64,
    piping_rate_hz: f64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.queen_loss_likely { 1.0 } else { 0.0 },
        if r.swarming_likely { 1.0 } else { 0.0 },
        if r.robbing_likely { 1.0 } else { 0.0 },
        r.hum_energy.min(1.0),
        (r.chaos_z.abs() / 5.0).clamp(0.0, 1.0),
        (r.piping_rate_hz / 1.0).clamp(0.0, 1.0),
        0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[14, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut c = std::net::TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new(); c.read_to_end(&mut resp).ok();
    Ok(())
}

fn autocorr_peak_hz(history: &VecDeque<f64>, sr: f64, min_hz: f64, max_hz: f64) -> f64 {
    if history.len() < 30 { return 0.0; }
    let n = history.len();
    let mean: f64 = history.iter().sum::<f64>() / n as f64;
    let centered: Vec<f64> = history.iter().map(|v| v - mean).collect();
    let var: f64 = centered.iter().map(|v| v * v).sum::<f64>().max(1e-12);
    let lag_min = (sr / max_hz).max(1.0) as usize;
    let lag_max = (sr / min_hz).min(n as f64 / 2.0) as usize;
    if lag_min >= lag_max { return 0.0; }
    let mut best_lag = 0usize;
    let mut best_corr = -1.0f64;
    for lag in lag_min..=lag_max {
        let mut s = 0.0;
        for i in 0..(n - lag) { s += centered[i] * centered[i + lag]; }
        let corr = s / var;
        if corr > best_corr { best_corr = corr; best_lag = lag; }
    }
    if best_lag == 0 || best_corr < 0.4 { 0.0 } else { sr / best_lag as f64 }
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(10);
    let chaos_z_thr: f64 = parse_arg(&args, "--chaos-z").unwrap_or(1.5);
    let robbing_z_thr: f64 = parse_arg(&args, "--robbing-z").unwrap_or(3.0);

    eprintln!("[cog-beehive-monitor] start (interval={interval}s, chaos_z={chaos_z_thr}, robbing_z={robbing_z_thr})");

    let mut hum_baseline = Welford::new();
    let mut chaos_baseline = Welford::new();
    let mut energy_history: VecDeque<f64> = VecDeque::with_capacity(60);
    let sample_rate = 1.0 / interval as f64;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        let mid = (amps.len() / 2).max(1);
                        let hum: f64 = amps[..mid].iter().map(|v| v.abs()).sum::<f64>() / mid as f64;
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let chaos: f64 = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;
                        let total: f64 = amps.iter().map(|v| v.abs()).sum::<f64>() / amps.len() as f64;

                        energy_history.push_back(total);
                        while energy_history.len() > 60 { energy_history.pop_front(); }

                        let z_chaos = chaos_baseline.z(chaos);
                        let z_hum = hum_baseline.z(hum);
                        let piping_hz = autocorr_peak_hz(&energy_history, sample_rate, 0.05, 0.3);

                        let queenless = z_chaos > chaos_z_thr && z_hum > 0.5;
                        let swarming = piping_hz > 0.0;
                        let robbing = z_hum > robbing_z_thr;

                        // Update baselines on healthy frames
                        if !queenless && !swarming && !robbing {
                            hum_baseline.update(hum);
                            chaos_baseline.update(chaos);
                        }

                        let status = if robbing { "ROBBING" }
                                     else if swarming { "SWARMING" }
                                     else if queenless { "QUEENLESS" }
                                     else if z_chaos > chaos_z_thr { "chaotic" }
                                     else { "healthy" };

                        let r = Report {
                            status: status.into(),
                            queen_loss_likely: queenless,
                            swarming_likely: swarming,
                            robbing_likely: robbing,
                            hum_energy: hum,
                            chaos_z: z_chaos,
                            piping_rate_hz: piping_hz,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-beehive-monitor] store error: {e}"); }
                        if robbing { eprintln!("[cog-beehive-monitor] ALERT: robbing event"); }
                        else if queenless { eprintln!("[cog-beehive-monitor] WARNING: queen-loss likely"); }
                        else if swarming { eprintln!("[cog-beehive-monitor] WARNING: swarming preparation likely"); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-beehive-monitor] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
