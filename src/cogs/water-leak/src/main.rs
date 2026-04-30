//! Cognitum Cog: Water Leak Detection (ADR-011)

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
    leak_likely: bool,
    leak_confirmed: bool,
    hiss_z: f64,
    drip_rate_hz: f64,
    persistence_secs: f64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.leak_likely { 1.0 } else { 0.0 },
        if r.leak_confirmed { 1.0 } else { 0.0 },
        (r.hiss_z / 4.0).clamp(0.0, 1.0),
        (r.drip_rate_hz / 5.0).clamp(0.0, 1.0),
        (r.persistence_secs / 3600.0).min(1.0),
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[11, v]], "dedup": true });
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
    if best_lag == 0 || best_corr < 0.3 { 0.0 } else { sr / best_lag as f64 }
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(2);
    let hiss_z_thr: f64 = parse_arg(&args, "--hiss-z").unwrap_or(1.5);
    let persistence_secs: u64 = parse_arg(&args, "--persistence").unwrap_or(300);

    eprintln!("[cog-water-leak] start (interval={interval}s, hiss_z={hiss_z_thr}, persistence={persistence_secs}s)");

    let mut baseline = Welford::new();
    let mut history: VecDeque<f64> = VecDeque::with_capacity(150);
    let mut signature_since: Option<Instant> = None;
    let sample_rate = 1.0 / interval as f64;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;
                        let z = baseline.z(var);

                        history.push_back(var);
                        while history.len() > 150 { history.pop_front(); }

                        let drip_hz = autocorr_peak_hz(&history, sample_rate, 1.0, 3.0);
                        let hiss_present = z > hiss_z_thr;
                        let drip_present = drip_hz > 0.0;
                        let signature = hiss_present || drip_present;

                        let now = Instant::now();
                        if signature {
                            if signature_since.is_none() { signature_since = Some(now); }
                        } else {
                            signature_since = None;
                            baseline.update(var);
                        }

                        let persistence = signature_since.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(0.0);
                        let leak_likely = signature && persistence >= 30.0;
                        let leak_confirmed = signature && persistence >= persistence_secs as f64;

                        let status = if leak_confirmed { "LEAK_CONFIRMED" }
                                     else if leak_likely { "LEAK_LIKELY" }
                                     else if drip_present { "drip" }
                                     else if hiss_present { "hiss" }
                                     else { "dry" };

                        let r = Report {
                            status: status.into(),
                            leak_likely,
                            leak_confirmed,
                            hiss_z: z,
                            drip_rate_hz: drip_hz,
                            persistence_secs: persistence,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-water-leak] store error: {e}"); }
                        if leak_confirmed { eprintln!("[cog-water-leak] ALERT: leak confirmed (hiss_z={:.1}, drip_rate={:.2}Hz, sustained={:.0}s)", z, drip_hz, persistence); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-water-leak] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
