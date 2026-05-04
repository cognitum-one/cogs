//! Cognitum Cog: Frost Warning (ADR-013)
//!
//! Linear-trend extrapolation of temperature with dewpoint-depression
//! gate. Channel 0 (or labeled `temp_c`) is treated as temperature in
//! Celsius; channel 1 as dewpoint in Celsius (when present).

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

#[derive(serde::Serialize)]
struct Report {
    status: String,
    frost_likely: bool,
    frost_confirmed: bool,
    current_temp_c: f64,
    projected_temp_c_at_h: f64,
    trend_c_per_h: f64,
    dewpoint_depression_c: f64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.frost_confirmed { 1.0 } else if r.frost_likely { 0.6 } else { 0.0 },
        (r.current_temp_c.abs() / 50.0).clamp(0.0, 1.0),
        ((r.projected_temp_c_at_h + 30.0) / 60.0).clamp(0.0, 1.0),
        ((r.trend_c_per_h + 5.0) / 10.0).clamp(0.0, 1.0),
        (r.dewpoint_depression_c / 30.0).clamp(0.0, 1.0),
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[13, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut c = std::net::TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new(); c.read_to_end(&mut resp).ok();
    Ok(())
}

/// Linear least-squares slope of `points` (assumed evenly-spaced in time).
/// Returns (slope per index, intercept).
fn lsq_slope(points: &VecDeque<f64>) -> (f64, f64) {
    let n = points.len();
    if n < 2 { return (0.0, points.front().copied().unwrap_or(0.0)); }
    let nf = n as f64;
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let mean_x = (n - 1) as f64 / 2.0;
    let mean_y: f64 = points.iter().sum::<f64>() / nf;
    let mut num = 0.0;
    let mut den = 0.0;
    for (x, y) in xs.iter().zip(points.iter()) {
        num += (x - mean_x) * (y - mean_y);
        den += (x - mean_x).powi(2);
    }
    let slope = if den > 1e-12 { num / den } else { 0.0 };
    let intercept = mean_y - slope * mean_x;
    (slope, intercept)
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(10);
    let frost_threshold: f64 = parse_arg(&args, "--frost-threshold").unwrap_or(2.0);
    let projection_hours: u64 = parse_arg(&args, "--projection-hours").unwrap_or(6);
    let dewpoint_min_depression: f64 = parse_arg(&args, "--dewpoint-min-depression").unwrap_or(3.0);

    eprintln!("[cog-frost-warning] start (interval={interval}s, threshold={frost_threshold}C, projection={projection_hours}h, depression={dewpoint_min_depression}C)");

    let window_secs = 4 * 3600;
    let max_samples = (window_secs as u64 / interval.max(1)) as usize;
    let mut history: VecDeque<f64> = VecDeque::with_capacity(max_samples);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                if let Some(chs) = s.get("samples").and_then(|c| c.as_array()) {
                    // Channel 0 = temp_c proxy. Channel 1 = dewpoint_c proxy if present.
                    let temp_c = chs.first().and_then(|c| c.get("value").and_then(|v| v.as_f64())).unwrap_or(0.0);
                    let dewpoint_c = chs.get(1).and_then(|c| c.get("value").and_then(|v| v.as_f64())).unwrap_or(temp_c - 5.0);

                    history.push_back(temp_c);
                    while history.len() > max_samples { history.pop_front(); }

                    let (slope_per_step, _) = lsq_slope(&history);
                    let trend_c_per_h = slope_per_step * (3600.0 / interval as f64);
                    let projected = temp_c + trend_c_per_h * projection_hours as f64;
                    let depression = temp_c - dewpoint_c;

                    let frost_confirmed = temp_c < frost_threshold;
                    let frost_likely = !frost_confirmed && projected < frost_threshold && depression < dewpoint_min_depression && history.len() >= 30;

                    let status = if frost_confirmed { "FROST_CONFIRMED" }
                                 else if frost_likely { "FROST_LIKELY" }
                                 else if trend_c_per_h < -0.5 { "cooling" }
                                 else { "warm" };

                    let r = Report {
                        status: status.into(),
                        frost_likely, frost_confirmed,
                        current_temp_c: temp_c,
                        projected_temp_c_at_h: projected,
                        trend_c_per_h,
                        dewpoint_depression_c: depression,
                        timestamp: now_ts(),
                    };
                    println!("{}", serde_json::to_string(&r).unwrap_or_default());
                    if let Err(e) = store(&r) { eprintln!("[cog-frost-warning] store error: {e}"); }
                    if frost_confirmed { eprintln!("[cog-frost-warning] ALERT: frost confirmed (current={:.1}C)", temp_c); }
                    else if frost_likely { eprintln!("[cog-frost-warning] WARNING: frost likely in {}h (projected={:.1}C, depression={:.1}C)", projection_hours, projected, depression); }
                }
            }
            Err(e) => eprintln!("[cog-frost-warning] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
