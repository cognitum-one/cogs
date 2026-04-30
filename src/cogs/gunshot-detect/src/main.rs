//! Cognitum Cog: Gunshot Detection (ADR-007)
//!
//! Saturating peak + exponential decay detector. Optional ruview-mode
//! reinforcement via CSI variance drop in post-peak window.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase { Quiet, Decaying { since: Instant, peak: f64, frames: u32, csi_baseline: f64 }, Cooldown { until: Instant } }

#[derive(serde::Serialize)]
struct Report {
    status: String,
    gunshot_detected: bool,
    peak_amplitude: f64,
    decay_score: f64,
    ruview_motion_drop: bool,
    total_shots: u64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.gunshot_detected { 1.0 } else { 0.0 },
        r.peak_amplitude.abs().min(1.0),
        r.decay_score.min(1.0),
        if r.ruview_motion_drop { 1.0 } else { 0.0 },
        (r.total_shots as f64 / 100.0).min(1.0),
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[7, v]], "dedup": true });
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
    let peak_threshold: f64 = parse_arg(&args, "--peak-threshold").unwrap_or(0.95);
    let decay_frames: u32 = parse_arg(&args, "--decay-frames").unwrap_or(4);
    let cooldown_secs: u64 = parse_arg(&args, "--cooldown").unwrap_or(30);
    let ruview_mode = args.iter().any(|a| a == "--ruview-mode");

    eprintln!("[cog-gunshot-detect] start (interval={interval}s, peak={peak_threshold}, decay={decay_frames}, ruview={ruview_mode})");

    let mut phase = Phase::Quiet;
    let mut total: u64 = 0;
    let mut csi_history: VecDeque<f64> = VecDeque::with_capacity(20);
    let mut last_peak = 0.0;
    let mut last_decay_score = 0.0;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        let peak = amps.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;

                        csi_history.push_back(var);
                        while csi_history.len() > 20 { csi_history.pop_front(); }

                        let now = Instant::now();
                        let mut fired = false;
                        let mut motion_drop = false;

                        phase = match phase {
                            Phase::Cooldown { until } if now < until => Phase::Cooldown { until },
                            Phase::Cooldown { .. } => Phase::Quiet,
                            Phase::Quiet => {
                                if peak >= peak_threshold {
                                    let csi_baseline = csi_history.iter().take(csi_history.len().saturating_sub(1)).sum::<f64>() / csi_history.len().max(1) as f64;
                                    Phase::Decaying { since: now, peak, frames: 0, csi_baseline }
                                } else { Phase::Quiet }
                            }
                            Phase::Decaying { since, peak: p, frames, csi_baseline } => {
                                let next_frames = frames + 1;
                                let expected_decay = p * (-0.5 * next_frames as f64).exp();
                                let observed = peak;
                                let decay_match = observed < expected_decay * 1.4 && observed > expected_decay * 0.3;

                                if next_frames >= decay_frames {
                                    let decay_score = if decay_match { 0.7 } else { 0.3 };
                                    last_decay_score = decay_score;
                                    last_peak = p;

                                    // RuView reinforcement: variance drop in 5s after peak
                                    if ruview_mode && since.elapsed() < Duration::from_secs(5) {
                                        let recent_var: f64 = csi_history.iter().rev().take(3).sum::<f64>() / 3.0;
                                        if csi_baseline > 1e-9 && recent_var < csi_baseline * 0.4 { motion_drop = true; }
                                    }

                                    let total_score = decay_score + if motion_drop { 0.25 } else { 0.0 };
                                    if total_score >= 0.6 {
                                        total += 1;
                                        fired = true;
                                        Phase::Cooldown { until: now + Duration::from_secs(cooldown_secs) }
                                    } else { Phase::Quiet }
                                } else if !decay_match && next_frames >= 2 {
                                    Phase::Quiet
                                } else {
                                    Phase::Decaying { since, peak: p, frames: next_frames, csi_baseline }
                                }
                            }
                        };

                        let status = match phase {
                            Phase::Cooldown { .. } if fired => "GUNSHOT",
                            Phase::Cooldown { .. } => "cooldown",
                            Phase::Decaying { .. } => "peak",
                            Phase::Quiet => "quiet",
                        };

                        let r = Report {
                            status: status.into(),
                            gunshot_detected: fired,
                            peak_amplitude: if fired { last_peak } else { peak },
                            decay_score: if fired { last_decay_score } else { 0.0 },
                            ruview_motion_drop: motion_drop,
                            total_shots: total,
                            timestamp: now_ts(),
                        };
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        if let Err(e) = store(&r) { eprintln!("[cog-gunshot-detect] store error: {e}"); }
                        if fired { eprintln!("[cog-gunshot-detect] ALERT: gunshot detected (peak={:.2}, decay={:.2}, motion_drop={motion_drop})", last_peak, last_decay_score); }
                    }
                }
            }
            Err(e) => eprintln!("[cog-gunshot-detect] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
