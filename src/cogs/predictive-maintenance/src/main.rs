//! Cognitum Cog: Predictive Maintenance (ADR-015)
//!
//! Vibration harmonic analyzer for rotating equipment. Hand-coded
//! radix-2 DFT (no external FFT crate).

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const N_FFT: usize = 64;

#[derive(serde::Serialize)]
struct Report {
    status: String,
    alarm: bool,
    severity_score: f64,
    imbalance_pct: f64,
    misalignment_pct: f64,
    bearing_pct: f64,
    looseness_pct: f64,
    baseline_complete: bool,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.alarm { 1.0 } else { 0.0 },
        r.severity_score.min(1.0),
        r.imbalance_pct.min(1.0),
        r.misalignment_pct.min(1.0),
        r.bearing_pct.min(1.0),
        r.looseness_pct.min(1.0),
        if r.baseline_complete { 1.0 } else { 0.0 },
        0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[15, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut c = std::net::TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new(); c.read_to_end(&mut resp).ok();
    Ok(())
}

/// Naive DFT magnitudes for an N-point real input. O(N²) but N=64 so
/// ~4 K multiply-adds per frame — well within budget for a 1 Hz cog.
fn dft_magnitudes(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut mags = Vec::with_capacity(n / 2);
    let two_pi = std::f64::consts::TAU;
    for k in 0..n / 2 {
        let mut re = 0.0;
        let mut im = 0.0;
        for (j, &v) in x.iter().enumerate() {
            let theta = two_pi * k as f64 * j as f64 / n as f64;
            re += v * theta.cos();
            im -= v * theta.sin();
        }
        mags.push((re * re + im * im).sqrt() / n as f64);
    }
    mags
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(1);
    let baseline_mins: u64 = parse_arg(&args, "--baseline-mins").unwrap_or(5);
    let severity_warn: f64 = parse_arg(&args, "--severity-warn").unwrap_or(0.4);
    let severity_alarm: f64 = parse_arg(&args, "--severity-alarm").unwrap_or(0.7);

    eprintln!("[cog-predictive-maintenance] start (interval={interval}s, baseline={baseline_mins}min, warn={severity_warn}, alarm={severity_alarm})");

    let baseline_samples = (baseline_mins * 60 / interval.max(1)) as usize;
    let mut accumulator: VecDeque<f64> = VecDeque::with_capacity(N_FFT * 2);
    let mut baseline_mags: Vec<f64> = Vec::new();
    let mut baseline_count: usize = 0;
    let mut baseline_complete = false;
    let mut f1_bin = 1usize; // estimated rotation bin

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        // Build a synthetic time series by appending the latest samples.
                        for &a in amps.iter().take(N_FFT) { accumulator.push_back(a); }
                        while accumulator.len() > N_FFT { accumulator.pop_front(); }

                        if accumulator.len() == N_FFT {
                            let frame: Vec<f64> = accumulator.iter().copied().collect();
                            let mags = dft_magnitudes(&frame);

                            // Baseline phase
                            if !baseline_complete {
                                if baseline_mags.is_empty() { baseline_mags = vec![0.0; mags.len()]; }
                                for (b, m) in baseline_mags.iter_mut().zip(mags.iter()) { *b += *m; }
                                baseline_count += 1;
                                if baseline_count >= baseline_samples {
                                    for b in baseline_mags.iter_mut() { *b /= baseline_count as f64; }
                                    f1_bin = (1..mags.len()).max_by(|&a, &b| baseline_mags[a].partial_cmp(&baseline_mags[b]).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(1);
                                    baseline_complete = true;
                                }
                                let r = Report { status: "learning".into(), alarm: false, severity_score: 0.0, imbalance_pct: 0.0, misalignment_pct: 0.0, bearing_pct: 0.0, looseness_pct: 0.0, baseline_complete: false, timestamp: now_ts() };
                                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                                if let Err(e) = store(&r) { eprintln!("[cog-predictive-maintenance] store error: {e}"); }
                            } else {
                                // Compute metric ratios
                                let baseline_f1 = baseline_mags.get(f1_bin).copied().unwrap_or(1e-9).max(1e-9);
                                let cur_f1 = mags.get(f1_bin).copied().unwrap_or(0.0);
                                let imbalance = ((cur_f1 - baseline_f1) / baseline_f1).abs().min(1.0);

                                let bin_2x = (f1_bin * 2).min(mags.len() - 1);
                                let baseline_2x = baseline_mags.get(bin_2x).copied().unwrap_or(1e-9).max(1e-9);
                                let cur_2x = mags.get(bin_2x).copied().unwrap_or(0.0);
                                let misalignment = ((cur_2x - baseline_2x) / baseline_2x).abs().min(1.0);

                                // High-order: bins 3×F1 .. 10×F1
                                let mut hi_baseline = 0.0;
                                let mut hi_cur = 0.0;
                                for h in 3..=10 {
                                    let b = (f1_bin * h).min(mags.len() - 1);
                                    hi_baseline += baseline_mags[b];
                                    hi_cur += mags[b];
                                }
                                let bearing = if hi_baseline > 1e-9 { ((hi_cur - hi_baseline) / hi_baseline).abs().min(1.0) } else { 0.0 };

                                // Sideband near F1 (looseness)
                                let lo = f1_bin.saturating_sub(2);
                                let hi = (f1_bin + 2).min(mags.len() - 1);
                                let mut side_baseline = 0.0;
                                let mut side_cur = 0.0;
                                for b in lo..=hi { if b != f1_bin { side_baseline += baseline_mags[b]; side_cur += mags[b]; } }
                                let looseness = if side_baseline > 1e-9 { ((side_cur - side_baseline) / side_baseline).abs().min(1.0) } else { 0.0 };

                                let severity = (imbalance * 0.30 + misalignment * 0.25 + bearing * 0.30 + looseness * 0.15).min(1.0);
                                let alarm = severity >= severity_alarm;
                                let warn = severity >= severity_warn;
                                let status = if alarm { "ALARM" }
                                             else if warn { "warn" }
                                             else { "healthy" };

                                let r = Report {
                                    status: status.into(),
                                    alarm,
                                    severity_score: severity,
                                    imbalance_pct: imbalance,
                                    misalignment_pct: misalignment,
                                    bearing_pct: bearing,
                                    looseness_pct: looseness,
                                    baseline_complete: true,
                                    timestamp: now_ts(),
                                };
                                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                                if let Err(e) = store(&r) { eprintln!("[cog-predictive-maintenance] store error: {e}"); }
                                if alarm { eprintln!("[cog-predictive-maintenance] ALARM: severity={:.2} (imb={:.2}, mis={:.2}, brg={:.2}, loose={:.2})", severity, imbalance, misalignment, bearing, looseness); }
                            }
                        } else {
                            // Not enough samples yet
                            let r = Report { status: "learning".into(), alarm: false, severity_score: 0.0, imbalance_pct: 0.0, misalignment_pct: 0.0, bearing_pct: 0.0, looseness_pct: 0.0, baseline_complete: false, timestamp: now_ts() };
                            println!("{}", serde_json::to_string(&r).unwrap_or_default());
                        }
                    }
                }
            }
            Err(e) => eprintln!("[cog-predictive-maintenance] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
