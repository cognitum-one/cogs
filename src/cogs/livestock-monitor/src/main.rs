//! Cognitum Cog: Livestock Monitor
//!
//! Detects animal activity patterns from CSI. Animals produce lower-frequency,
//! different-amplitude patterns vs humans. Alerts on distress (erratic movement)
//! or escape (sudden absence after sustained presence).
//!
//! Usage:
//!   cog-livestock-monitor --once
//!   cog-livestock-monitor --interval 5

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const PRESENCE_THRESHOLD: f64 = 3.0;
const DISTRESS_VARIANCE_RATIO: f64 = 5.0;
const ESCAPE_ABSENCE_FRAMES: u32 = 6;

struct LivestockState {
    baseline_mean: f64,
    baseline_var: f64,
    baseline_count: u64,
    presence_streak: u32,
    absence_streak: u32,
    was_present: bool,
    variance_history: Vec<f64>,
    max_history: usize,
}

impl LivestockState {
    fn new() -> Self {
        Self {
            baseline_mean: 0.0,
            baseline_var: 0.0,
            baseline_count: 0,
            presence_streak: 0,
            absence_streak: 0,
            was_present: false,
            variance_history: Vec::new(),
            max_history: 20,
        }
    }

    fn update_baseline(&mut self, val: f64) {
        self.baseline_count += 1;
        let delta = val - self.baseline_mean;
        self.baseline_mean += delta / self.baseline_count as f64;
        let delta2 = val - self.baseline_mean;
        self.baseline_var += delta * delta2;
    }

    fn current_baseline_var(&self) -> f64 {
        if self.baseline_count < 2 { 1.0 }
        else { (self.baseline_var / (self.baseline_count - 1) as f64).max(0.01) }
    }
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match conn.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => { buf.extend_from_slice(&tmp[..n]); if buf.len() > 262144 { break; } }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(vec: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec.to_vec()]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

#[derive(serde::Serialize)]
struct LivestockReport {
    animal_present: bool,
    activity_level: String,   // "none", "resting", "normal", "active", "erratic"
    signal_variance: f64,
    baseline_variance: f64,
    variance_ratio: f64,
    distress_detected: bool,
    escape_detected: bool,
    presence_streak: u32,
    alert: bool,
    alert_reason: Option<String>,
    timestamp: u64,
}

fn run_once(state: &mut LivestockState) -> Result<LivestockReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    if values.len() < 2 {
        return Err("insufficient data".into());
    }

    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let variance: f64 = values.iter()
        .map(|v| (v - mean) * (v - mean))
        .sum::<f64>() / values.len() as f64;

    let baseline_var = state.current_baseline_var();
    let ratio = variance / baseline_var;

    // Track variance history for erratic detection
    state.variance_history.push(variance);
    if state.variance_history.len() > state.max_history {
        state.variance_history.remove(0);
    }

    // Animal presence: lower frequency, moderate amplitude
    let present = ratio > PRESENCE_THRESHOLD;

    if present {
        state.presence_streak += 1;
        state.absence_streak = 0;
    } else {
        state.absence_streak += 1;
        state.presence_streak = 0;
        state.update_baseline(variance);
    }

    // Distress: high variance of variance (erratic movement)
    let distress = if state.variance_history.len() >= 5 {
        let var_mean: f64 = state.variance_history.iter().sum::<f64>()
            / state.variance_history.len() as f64;
        let var_of_var: f64 = state.variance_history.iter()
            .map(|v| (v - var_mean) * (v - var_mean))
            .sum::<f64>() / state.variance_history.len() as f64;
        var_of_var.sqrt() / var_mean.max(0.01) > DISTRESS_VARIANCE_RATIO
    } else { false };

    // Escape: was present, now suddenly absent
    let escape = state.was_present && state.absence_streak >= ESCAPE_ABSENCE_FRAMES;
    state.was_present = present;

    let activity_level = if !present { "none" }
    else if distress { "erratic" }
    else if ratio > 10.0 { "active" }
    else if ratio > 5.0 { "normal" }
    else { "resting" };

    let alert = distress || escape;
    let alert_reason = if distress {
        Some("Erratic movement detected — possible distress".to_string())
    } else if escape {
        Some("Sudden absence after sustained presence — possible escape".to_string())
    } else { None };

    let vector = [
        if present { 1.0 } else { 0.0 },
        match activity_level { "erratic" => 1.0, "active" => 0.75, "normal" => 0.5, "resting" => 0.25, _ => 0.0 },
        (variance / 100.0).min(1.0),
        ratio.min(20.0) / 20.0,
        if distress { 1.0 } else { 0.0 },
        if escape { 1.0 } else { 0.0 },
        state.presence_streak as f64 / 100.0,
        if alert { 1.0 } else { 0.0 },
    ];
    let _ = store_vector(&vector);

    Ok(LivestockReport {
        animal_present: present,
        activity_level: activity_level.to_string(),
        signal_variance: variance,
        baseline_variance: baseline_var,
        variance_ratio: ratio,
        distress_detected: distress,
        escape_detected: escape,
        presence_streak: state.presence_streak,
        alert,
        alert_reason,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5);

    eprintln!("[cog-livestock-monitor] starting (interval={interval}s)");
    let mut state = LivestockState::new();

    loop {
        let start = Instant::now();
        match run_once(&mut state) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.alert {
                    eprintln!("[cog-livestock-monitor] ALERT: {}", report.alert_reason.as_deref().unwrap_or("unknown"));
                }
            }
            Err(e) => eprintln!("[cog-livestock-monitor] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
