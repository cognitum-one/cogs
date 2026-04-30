//! Cognitum Cog: Baby Cry Detection (ADR-004)
//!
//! Sustained mid-band energy detector. Fires when mid-band energy exceeds
//! baseline by `cry_z` for `cry_min_secs` of consecutive samples.

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
struct Report { status: String, cry_detected: bool, sustained_secs: f64, midband_z: f64, total_cries: u64, timestamp: u64 }

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.cry_detected { 1.0 } else { 0.0 },
        (r.sustained_secs / 30.0).min(1.0),
        (r.midband_z / 5.0).clamp(0.0, 1.0),
        (r.total_cries as f64 / 100.0).min(1.0),
        0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[4, v]], "dedup": true });
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
    let cry_z: f64 = parse_arg(&args, "--cry-z").unwrap_or(2.5);
    let cry_min_secs: u64 = parse_arg(&args, "--cry-min-secs").unwrap_or(2);
    let cooldown_secs: u64 = parse_arg(&args, "--cooldown").unwrap_or(15);

    eprintln!("[cog-baby-cry] start (interval={interval}s, z={cry_z}, min={cry_min_secs}s, cooldown={cooldown_secs}s)");

    let mut baseline = Welford::new();
    let mut elevated_secs: f64 = 0.0;
    let mut total: u64 = 0;
    let mut cooldown_until: Option<Instant> = None;

    loop {
        let start = Instant::now();
        let now = Instant::now();
        let in_cooldown = cooldown_until.map(|t| now < t).unwrap_or(false);
        if !in_cooldown { cooldown_until = None; }

        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if amps.len() >= 4 {
                        // Mid-band proxy: weighted second-half energy
                        let mid = amps.len() / 2;
                        let mid_energy: f64 = amps[mid..].iter().map(|v| v.abs()).sum::<f64>() / (amps.len() - mid) as f64;
                        let z = baseline.z(mid_energy);
                        let elevated = !in_cooldown && z > cry_z;
                        if elevated { elevated_secs += interval as f64; }
                        else { elevated_secs = 0.0; baseline.update(mid_energy); }

                        let mut fired = false;
                        let status = if in_cooldown { "cooldown" }
                                     else if elevated_secs >= cry_min_secs as f64 {
                                         total += 1;
                                         elevated_secs = 0.0;
                                         cooldown_until = Some(now + Duration::from_secs(cooldown_secs));
                                         fired = true;
                                         "CRY_DETECTED"
                                     } else if elevated { "elevated" } else { "quiet" };

                        let r = Report { status: status.into(), cry_detected: fired, sustained_secs: elevated_secs, midband_z: z, total_cries: total, timestamp: now_ts() };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-baby-cry] store error: {e}"); }
                        if fired { eprintln!("[cog-baby-cry] ALERT: baby cry detected (z={:.1})", z); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-baby-cry] sensor error: {e}"),
        }

        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
