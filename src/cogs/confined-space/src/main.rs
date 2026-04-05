//! Cognitum Cog: Confined Space Monitor
//!
//! Monitors workers in confined spaces. Tracks presence duration and
//! breathing-rate proxy (low-frequency signal oscillation). Alerts if
//! no movement for >60s or breathing anomaly detected.
//!
//! Usage:
//!   cog-confined-space --once
//!   cog-confined-space --interval 2

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const INACTIVITY_THRESHOLD_SECS: u64 = 60;
const BREATHING_LOW_BPM: f64 = 8.0;
const BREATHING_HIGH_BPM: f64 = 30.0;
const MOVEMENT_THRESHOLD: f64 = 2.0;

struct ConfinedState {
    presence_start: Option<Instant>,
    last_movement: Instant,
    signal_buffer: Vec<f64>,       // Recent signal values for breathing estimation
    buffer_max: usize,
    prev_variance: f64,
}

impl ConfinedState {
    fn new() -> Self {
        Self {
            presence_start: None,
            last_movement: Instant::now(),
            signal_buffer: Vec::new(),
            buffer_max: 60,  // ~60 samples at 1s interval
            prev_variance: 0.0,
        }
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
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

/// Estimate breathing rate from signal oscillation (zero-crossings)
fn estimate_breathing_rate(buffer: &[f64], sample_interval_secs: f64) -> f64 {
    if buffer.len() < 10 { return 0.0; }

    // Remove DC (mean)
    let mean: f64 = buffer.iter().sum::<f64>() / buffer.len() as f64;
    let centered: Vec<f64> = buffer.iter().map(|v| v - mean).collect();

    // Count zero crossings
    let mut crossings = 0u32;
    for i in 1..centered.len() {
        if (centered[i] >= 0.0 && centered[i - 1] < 0.0)
            || (centered[i] < 0.0 && centered[i - 1] >= 0.0)
        {
            crossings += 1;
        }
    }

    // Each full cycle = 2 crossings
    let total_time = buffer.len() as f64 * sample_interval_secs;
    let cycles = crossings as f64 / 2.0;
    let bpm = (cycles / total_time) * 60.0;
    bpm
}

fn store_report(vector: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vector.to_vec()]], "dedup": true });
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
struct ConfinedReport {
    worker_present: bool,
    presence_duration_secs: u64,
    inactivity_secs: u64,
    estimated_breathing_bpm: f64,
    breathing_normal: bool,
    movement_detected: bool,
    alert: bool,
    alert_reason: Option<String>,
    timestamp: u64,
}

fn run_once(state: &mut ConfinedState, sample_interval: f64) -> Result<ConfinedReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    // Compute current signal power (aggregate across channels)
    let mut power = 0.0f64;
    let mut ch0_val = 0.0f64;
    for (i, sample) in samples.iter().enumerate() {
        let val = sample.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        power += val * val;
        if i == 0 { ch0_val = val; }
    }
    power = power.sqrt();

    // Add to breathing buffer (use channel 0 as breathing proxy)
    state.signal_buffer.push(ch0_val);
    if state.signal_buffer.len() > state.buffer_max {
        state.signal_buffer.remove(0);
    }

    // Detect movement: variance change
    let variance: f64 = if !samples.is_empty() {
        let mean = samples.iter()
            .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
            .sum::<f64>() / samples.len() as f64;
        samples.iter()
            .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
            .map(|v| (v - mean) * (v - mean))
            .sum::<f64>() / samples.len() as f64
    } else { 0.0 };

    let movement = (variance - state.prev_variance).abs() > MOVEMENT_THRESHOLD;
    state.prev_variance = variance;

    if movement {
        state.last_movement = Instant::now();
    }

    // Detect presence
    let worker_present = power > 1.0;  // Basic presence threshold
    if worker_present && state.presence_start.is_none() {
        state.presence_start = Some(Instant::now());
    } else if !worker_present {
        state.presence_start = None;
    }

    let presence_duration = state.presence_start
        .map(|s| s.elapsed().as_secs())
        .unwrap_or(0);
    let inactivity = state.last_movement.elapsed().as_secs();

    // Breathing rate estimation
    let breathing_bpm = estimate_breathing_rate(&state.signal_buffer, sample_interval);
    let breathing_normal = breathing_bpm >= BREATHING_LOW_BPM && breathing_bpm <= BREATHING_HIGH_BPM;

    // Alert logic
    let mut alert = false;
    let mut alert_reason = None;

    if worker_present && inactivity > INACTIVITY_THRESHOLD_SECS {
        alert = true;
        alert_reason = Some(format!("No movement for {inactivity}s"));
    } else if worker_present && state.signal_buffer.len() > 20 && !breathing_normal && breathing_bpm > 0.1 {
        alert = true;
        alert_reason = Some(format!("Abnormal breathing rate: {breathing_bpm:.1} BPM"));
    }

    let vector = [
        if worker_present { 1.0 } else { 0.0 },
        presence_duration as f64 / 3600.0,
        inactivity as f64 / 120.0,
        breathing_bpm / 30.0,
        if breathing_normal { 1.0 } else { 0.0 },
        if movement { 1.0 } else { 0.0 },
        power / 100.0,
        if alert { 1.0 } else { 0.0 },
    ];
    let _ = store_report(&vector);

    Ok(ConfinedReport {
        worker_present,
        presence_duration_secs: presence_duration,
        inactivity_secs: inactivity,
        estimated_breathing_bpm: breathing_bpm,
        breathing_normal,
        movement_detected: movement,
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
        .unwrap_or(2);

    eprintln!("[cog-confined-space] starting (interval={interval}s)");
    let mut state = ConfinedState::new();

    loop {
        let start = Instant::now();
        match run_once(&mut state, interval as f64) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.alert {
                    eprintln!("[cog-confined-space] ALERT: {}", report.alert_reason.as_deref().unwrap_or("unknown"));
                }
            }
            Err(e) => eprintln!("[cog-confined-space] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
