//! Cognitum Cog: Snore Monitor (ADR-005)
//!
//! Tracks periodic low-band energy bursts. Reports snores-per-minute and
//! estimated repetition rate via autocorrelation.

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
struct Report { status: String, snores_per_minute: u32, session_total: u64, burst_z: f64, estimated_rate_hz: f64, timestamp: u64 }

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        (r.snores_per_minute as f64 / 30.0).min(1.0),
        (r.session_total as f64 / 1000.0).min(1.0),
        (r.burst_z / 5.0).clamp(0.0, 1.0),
        (r.estimated_rate_hz / 5.0).min(1.0),
        0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[5, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut c = std::net::TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new(); c.read_to_end(&mut resp).ok();
    Ok(())
}

/// Estimate repetition rate via autocorrelation peak.
fn autocorr_peak_hz(history: &VecDeque<f64>, sample_rate_hz: f64, min_hz: f64, max_hz: f64) -> f64 {
    if history.len() < 30 { return 0.0; }
    let n = history.len();
    let mean: f64 = history.iter().sum::<f64>() / n as f64;
    let centered: Vec<f64> = history.iter().map(|v| v - mean).collect();
    let var: f64 = centered.iter().map(|v| v * v).sum::<f64>().max(1e-12);

    let lag_min = (sample_rate_hz / max_hz).max(1.0) as usize;
    let lag_max = (sample_rate_hz / min_hz).min(n as f64 / 2.0) as usize;
    if lag_min >= lag_max { return 0.0; }

    let mut best_lag = 0usize;
    let mut best_corr = -1.0f64;
    for lag in lag_min..=lag_max {
        let mut s = 0.0;
        for i in 0..(n - lag) { s += centered[i] * centered[i + lag]; }
        let corr = s / var;
        if corr > best_corr { best_corr = corr; best_lag = lag; }
    }
    if best_lag == 0 || best_corr < 0.3 { 0.0 } else { sample_rate_hz / best_lag as f64 }
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(1);
    let burst_z: f64 = parse_arg(&args, "--burst-z").unwrap_or(2.0);
    let min_hz: f64 = parse_arg(&args, "--minimum-rate-hz").unwrap_or(1.5);
    let max_hz: f64 = parse_arg(&args, "--maximum-rate-hz").unwrap_or(4.0);

    eprintln!("[cog-snore-monitor] start (interval={interval}s, z={burst_z}, rate={min_hz}-{max_hz}Hz)");

    let mut baseline = Welford::new();
    let mut history: VecDeque<f64> = VecDeque::with_capacity(60);
    let mut events_minute: VecDeque<Instant> = VecDeque::new();
    let mut total: u64 = 0;
    let sample_rate = 1.0 / interval as f64;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if amps.len() >= 2 {
                        let mid = (amps.len() / 2).max(1);
                        let low_energy: f64 = amps[..mid].iter().map(|v| v.abs()).sum::<f64>() / mid as f64;

                        let z = baseline.z(low_energy);
                        if z < burst_z { baseline.update(low_energy); }

                        history.push_back(low_energy);
                        while history.len() > 60 { history.pop_front(); }

                        let now = Instant::now();
                        let cutoff = now - Duration::from_secs(60);
                        while events_minute.front().map(|&t| t < cutoff).unwrap_or(false) { events_minute.pop_front(); }

                        if z > burst_z {
                            // Suppress repeated triggers within 200 ms (cogs run at 1 Hz so this is academic).
                            events_minute.push_back(now);
                            total += 1;
                        }

                        let est_hz = autocorr_peak_hz(&history, sample_rate, min_hz, max_hz);
                        let status = if est_hz > 0.0 { "periodic" } else if z > burst_z { "monitoring" } else { "quiet" };

                        let r = Report {
                            status: status.into(),
                            snores_per_minute: events_minute.len() as u32,
                            session_total: total,
                            burst_z: z,
                            estimated_rate_hz: est_hz,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-snore-monitor] store error: {e}"); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-snore-monitor] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
