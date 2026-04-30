//! Cognitum Cog: Package Arrival Detection (ADR-008)
//!
//! Tracks sustained CSI subcarrier-variance shifts to detect new static
//! objects (packages) entering and leaving the scene. Requires the
//! ESP32 ruview CSI feature stream.

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
enum State { Empty, Transient { since: Instant }, Present { since: Instant } }

#[derive(serde::Serialize)]
struct Report {
    status: String,
    package_present: bool,
    persistence_secs: f64,
    shift_z: f64,
    session_arrivals: u64,
    session_departures: u64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.package_present { 1.0 } else { 0.0 },
        (r.persistence_secs / 600.0).min(1.0),
        (r.shift_z.abs() / 5.0).clamp(0.0, 1.0),
        (r.session_arrivals as f64 / 100.0).min(1.0),
        (r.session_departures as f64 / 100.0).min(1.0),
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[8, v]], "dedup": true });
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
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(2);
    let persistence_secs: u64 = parse_arg(&args, "--persistence").unwrap_or(30);
    let shift_z_threshold: f64 = parse_arg(&args, "--shift-z").unwrap_or(2.5);

    eprintln!("[cog-package-detect] start (interval={interval}s, persistence={persistence_secs}s, shift_z={shift_z_threshold})");

    let mut baseline = Welford::new();
    let mut state = State::Empty;
    let mut arrivals: u64 = 0;
    let mut departures: u64 = 0;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if amps.len() >= 2 {
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;

                        let z = baseline.z(var);
                        let now = Instant::now();

                        // Update baseline only when state is steady (Empty or Present)
                        let new_state = match state {
                            State::Empty => {
                                if z > shift_z_threshold {
                                    State::Transient { since: now }
                                } else {
                                    baseline.update(var);
                                    State::Empty
                                }
                            }
                            State::Transient { since } => {
                                if z < shift_z_threshold * 0.5 {
                                    State::Empty
                                } else if now.duration_since(since).as_secs() >= persistence_secs {
                                    arrivals += 1;
                                    State::Present { since }
                                } else {
                                    State::Transient { since }
                                }
                            }
                            State::Present { since } => {
                                if z < shift_z_threshold * 0.5 {
                                    departures += 1;
                                    baseline = Welford::new(); // reset baseline after departure
                                    State::Empty
                                } else {
                                    State::Present { since }
                                }
                            }
                        };

                        let persistence = match new_state {
                            State::Present { since } => now.duration_since(since).as_secs_f64(),
                            State::Transient { since } => now.duration_since(since).as_secs_f64(),
                            State::Empty => 0.0,
                        };

                        let status = match (state, new_state) {
                            (_, State::Present { .. }) if matches!(state, State::Transient { .. }) => "PACKAGE_PRESENT",
                            (_, State::Empty) if matches!(state, State::Present { .. }) => "PACKAGE_TAKEN",
                            (_, State::Present { .. }) => "PACKAGE_PRESENT",
                            (_, State::Transient { .. }) => "transient",
                            (_, State::Empty) => "empty",
                        };

                        let r = Report {
                            status: status.into(),
                            package_present: matches!(new_state, State::Present { .. }),
                            persistence_secs: persistence,
                            shift_z: z,
                            session_arrivals: arrivals,
                            session_departures: departures,
                            timestamp: now_ts(),
                        };
                        state = new_state;
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-package-detect] store error: {e}"); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-package-detect] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
