//! Cognitum Cog: Respiratory Distress Monitor
//!
//! Monitors breathing rate AND effort. Uses bandpass 0.1-0.8 Hz, measures
//! amplitude consistency. Alerts on tachypnea (>25 bpm), apnea (<4 bpm),
//! Cheyne-Stokes pattern (amplitude oscillation via envelope modulation).
//!
//! Usage:
//!   cog-respiratory-distress --once
//!   cog-respiratory-distress --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

struct BandpassFilter {
    a1: f64, a2: f64, b0: f64, b2: f64,
    x1: f64, x2: f64, y1: f64, y2: f64,
}

impl BandpassFilter {
    fn new(freq_low: f64, freq_high: f64, sample_rate: f64) -> Self {
        let omega_low = 2.0 * std::f64::consts::PI * freq_low / sample_rate;
        let omega_high = 2.0 * std::f64::consts::PI * freq_high / sample_rate;
        let center = (omega_low + omega_high) / 2.0;
        let bandwidth = omega_high - omega_low;
        let r = 1.0 - bandwidth / 2.0;
        let r2 = r * r;
        Self {
            a1: -2.0 * r * center.cos(), a2: r2,
            b0: 1.0 - r2, b2: -(1.0 - r2),
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        }
    }
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = input;
        self.y2 = self.y1; self.y1 = output;
        output
    }
}

/// Count zero crossings to estimate breathing rate
fn zero_crossing_bpm(signal: &[f64], sample_rate: f64) -> f64 {
    if signal.len() < 4 { return 0.0; }
    let mut crossings = 0;
    for i in 1..signal.len() {
        if (signal[i - 1] >= 0.0 && signal[i] < 0.0) ||
           (signal[i - 1] < 0.0 && signal[i] >= 0.0) {
            crossings += 1;
        }
    }
    let duration_s = signal.len() as f64 / sample_rate;
    if duration_s < 0.1 { return 0.0; }
    (crossings as f64 / (2.0 * duration_s)) * 60.0
}

/// Compute the amplitude envelope using a simple moving-average rectifier
fn amplitude_envelope(signal: &[f64], window: usize) -> Vec<f64> {
    let rectified: Vec<f64> = signal.iter().map(|v| v.abs()).collect();
    let win = window.max(1);
    if rectified.len() < win { return rectified; }
    let mut env = Vec::with_capacity(rectified.len());
    let mut sum: f64 = rectified[..win].iter().sum();
    for i in 0..rectified.len() {
        if i >= win {
            sum += rectified[i];
            sum -= rectified[i - win];
        }
        env.push(sum / win as f64);
    }
    env
}

/// Detect Cheyne-Stokes pattern: periodic waxing/waning of amplitude envelope.
/// Returns (detected, modulation_depth) where modulation_depth is the ratio
/// of envelope variance to envelope mean (high = oscillating amplitude).
fn detect_cheyne_stokes(envelope: &[f64]) -> (bool, f64) {
    if envelope.len() < 10 { return (false, 0.0); }
    let mean = envelope.iter().sum::<f64>() / envelope.len() as f64;
    if mean < 1e-10 { return (false, 0.0); }
    let variance = envelope.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / envelope.len() as f64;
    let cv = variance.sqrt() / mean;

    // Check for periodicity in envelope via zero-crossing of (envelope - mean)
    let detrended: Vec<f64> = envelope.iter().map(|v| v - mean).collect();
    let mut crossings = 0;
    for i in 1..detrended.len() {
        if (detrended[i - 1] >= 0.0 && detrended[i] < 0.0) ||
           (detrended[i - 1] < 0.0 && detrended[i] >= 0.0) {
            crossings += 1;
        }
    }

    // Cheyne-Stokes: high CV (>0.3) AND periodic oscillation (2+ full cycles)
    let detected = cv > 0.30 && crossings >= 4;
    (detected, cv)
}

/// Measure breathing effort: amplitude consistency (low variance = easy breathing)
fn breathing_effort(envelope: &[f64]) -> f64 {
    if envelope.len() < 2 { return 0.0; }
    let mean = envelope.iter().sum::<f64>() / envelope.len() as f64;
    if mean < 1e-10 { return 0.0; }
    let variance = envelope.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / envelope.len() as f64;
    // Effort = normalized std dev; higher = more labored
    (variance.sqrt() / mean).min(1.0)
}

#[derive(serde::Serialize)]
struct RespiratoryReport {
    breathing_rate_bpm: f64,
    breathing_effort: f64,
    amplitude_mean: f64,
    cheyne_stokes_detected: bool,
    modulation_depth: f64,
    status: String,
    alerts: Vec<String>,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_report(report: &RespiratoryReport) -> Result<(), String> {
    let status_code = match report.status.as_str() {
        "normal" => 0.0, "tachypnea" => 0.5, "apnea" => 0.9,
        "cheyne_stokes" => 0.8, _ => 0.2,
    };
    let vector = vec![
        report.breathing_rate_bpm / 40.0,
        report.breathing_effort,
        report.amplitude_mean.min(1.0),
        report.modulation_depth.min(1.0),
        status_code,
        if report.cheyne_stokes_detected { 1.0 } else { 0.0 },
        if report.alerts.is_empty() { 0.0 } else { 1.0 },
        0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once() -> Result<RespiratoryReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let amplitudes: Vec<f64> = samples.iter()
        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
        .collect();
    if amplitudes.len() < 4 {
        return Err("insufficient sensor data".into());
    }

    let sample_rate = 10.0;
    // Bandpass 0.1-0.8 Hz for respiration
    let mut resp_filter = BandpassFilter::new(0.1, 0.8, sample_rate);
    let filtered: Vec<f64> = amplitudes.iter().map(|&v| resp_filter.process(v)).collect();

    let breathing_bpm = zero_crossing_bpm(&filtered, sample_rate);
    let envelope = amplitude_envelope(&filtered, 5);
    let effort = breathing_effort(&envelope);
    let amp_mean = envelope.iter().sum::<f64>() / envelope.len().max(1) as f64;
    let (cs_detected, mod_depth) = detect_cheyne_stokes(&envelope);

    let mut alerts = Vec::new();
    let status = if cs_detected {
        alerts.push(format!("CHEYNE_STOKES: modulation_depth={:.3}", mod_depth));
        "cheyne_stokes".to_string()
    } else if breathing_bpm > 0.0 && breathing_bpm < 4.0 {
        alerts.push("APNEA: breathing rate critically low (<4 bpm)".into());
        "apnea".to_string()
    } else if breathing_bpm > 25.0 {
        alerts.push(format!("TACHYPNEA: breathing rate={:.1} bpm (>25)", breathing_bpm));
        "tachypnea".to_string()
    } else {
        "normal".to_string()
    };

    if effort > 0.6 {
        alerts.push(format!("HIGH_EFFORT: breathing_effort={:.2}", effort));
    }

    Ok(RespiratoryReport {
        breathing_rate_bpm: breathing_bpm,
        breathing_effort: effort,
        amplitude_mean: amp_mean,
        cheyne_stokes_detected: cs_detected,
        modulation_depth: mod_depth,
        status,
        alerts,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
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

    eprintln!("[cog-respiratory-distress] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_report(&report) {
                    eprintln!("[cog-respiratory-distress] store error: {e}");
                }
                if !report.alerts.is_empty() {
                    eprintln!("[cog-respiratory-distress] ALERT: {:?}", report.alerts);
                }
            }
            Err(e) => eprintln!("[cog-respiratory-distress] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
