//! Cognitum Cog: Parking Occupancy (ADR-016)
//!
//! Per-zone CSI subcarrier-energy occupancy tracking. Each zone is a
//! disjoint subset of the feature-stream channels. Hysteresis prevents
//! flicker.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

#[derive(serde::Serialize)]
struct Report {
    status: String,
    occupied_count: u32,
    total_zones: u32,
    utilization_pct: f64,
    churn_per_hour: f64,
    zone_states: Vec<bool>,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        r.utilization_pct,
        (r.occupied_count as f64 / r.total_zones.max(1) as f64).min(1.0),
        r.churn_per_hour.min(60.0) / 60.0,
        (r.total_zones as f64 / 8.0).min(1.0),
        0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[16, v]], "dedup": true });
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
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(5);
    let zones: usize = parse_arg(&args, "--zones").unwrap_or(4);
    let threshold: f64 = parse_arg(&args, "--threshold").unwrap_or(0.4);

    eprintln!("[cog-parking-occupancy] start (interval={interval}s, zones={zones}, threshold={threshold})");

    let mut zone_baselines: Vec<f64> = vec![0.0; zones];
    let mut zone_states: Vec<bool> = vec![false; zones];
    let mut state_history: VecDeque<(Instant, Vec<bool>)> = VecDeque::with_capacity(720);
    let mut baseline_count: u64 = 0;
    let baseline_alpha = 0.05;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        // Partition channels evenly among zones
                        let chans_per_zone = (amps.len() / zones).max(1);
                        let zone_energies: Vec<f64> = (0..zones).map(|z| {
                            let lo = z * chans_per_zone;
                            let hi = ((z + 1) * chans_per_zone).min(amps.len());
                            if hi > lo {
                                amps[lo..hi].iter().map(|v| v.abs()).sum::<f64>() / (hi - lo) as f64
                            } else { 0.0 }
                        }).collect();

                        baseline_count += 1;
                        for i in 0..zones {
                            let e = zone_energies[i];
                            if baseline_count <= 30 {
                                zone_baselines[i] = (zone_baselines[i] * (baseline_count as f64 - 1.0) + e) / baseline_count as f64;
                            } else if !zone_states[i] {
                                // Slow EMA when zone is free (don't drift baseline upward when occupied)
                                zone_baselines[i] = zone_baselines[i] * (1.0 - baseline_alpha) + e * baseline_alpha;
                            }
                        }

                        // Update zone states with hysteresis
                        let mut new_states = zone_states.clone();
                        for i in 0..zones {
                            let baseline = zone_baselines[i].max(1e-9);
                            let ratio = (zone_energies[i] - baseline) / baseline;
                            // Enter occupied at threshold; exit at threshold * 0.5
                            let on = if zone_states[i] { ratio > threshold * 0.5 } else { ratio > threshold };
                            new_states[i] = on;
                        }
                        zone_states = new_states.clone();

                        let now = Instant::now();
                        state_history.push_back((now, zone_states.clone()));
                        let cutoff = now - Duration::from_secs(3600);
                        while state_history.front().map(|(t, _)| *t < cutoff).unwrap_or(false) { state_history.pop_front(); }

                        // Compute churn = number of state changes per zone in last hour, summed.
                        let mut churn = 0u32;
                        if state_history.len() >= 2 {
                            for w in state_history.iter().zip(state_history.iter().skip(1)) {
                                let (_, prev) = w.0;
                                let (_, cur) = w.1;
                                for i in 0..zones { if prev[i] != cur[i] { churn += 1; } }
                            }
                        }
                        // Normalize to per-hour
                        let hours = state_history.front().zip(state_history.back()).map(|((a, _), (b, _))| (*b - *a).as_secs_f64() / 3600.0).unwrap_or(0.0).max(0.001);
                        let churn_per_hour = churn as f64 / hours;

                        let occupied = zone_states.iter().filter(|&&s| s).count() as u32;
                        let utilization = occupied as f64 / zones as f64;

                        let r = Report {
                            status: "monitoring".into(),
                            occupied_count: occupied,
                            total_zones: zones as u32,
                            utilization_pct: utilization,
                            churn_per_hour,
                            zone_states: zone_states.clone(),
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-parking-occupancy] store error: {e}"); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-parking-occupancy] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
