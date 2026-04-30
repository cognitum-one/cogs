//! Cognitum Cog: Cardiac Arrhythmia Detector
//!
//! HRV analysis using RMSSD and pNN50 from bandpass-filtered heart rate
//! signal (0.8-2.5 Hz). Detects irregular R-R intervals via coefficient
//! of variation. Alerts on AF (irregular), tachycardia (>100 bpm),
//! bradycardia (<50 bpm).
//!
//! Usage:
//!   cog-cardiac-arrhythmia --once
//!   cog-cardiac-arrhythmia --interval 5

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

/// Detect R-peaks via threshold crossing on filtered signal.
/// Returns indices of detected peaks.
fn detect_r_peaks(signal: &[f64], threshold_factor: f64) -> Vec<usize> {
    if signal.len() < 3 { return vec![]; }

    // Adaptive threshold: fraction of max absolute amplitude
    let max_amp = signal.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    let threshold = max_amp * threshold_factor;
    if threshold < 1e-12 { return vec![]; }

    let mut peaks = Vec::new();
    for i in 1..signal.len() - 1 {
        if signal[i] > threshold && signal[i] > signal[i - 1] && signal[i] >= signal[i + 1] {
            // Enforce minimum distance (refractory period ~200ms at 10Hz = 2 samples)
            if peaks.last().map_or(true, |&last: &usize| i - last >= 2) {
                peaks.push(i);
            }
        }
    }
    peaks
}

/// Compute R-R intervals in milliseconds from peak indices
fn rr_intervals_ms(peaks: &[usize], sample_rate: f64) -> Vec<f64> {
    if peaks.len() < 2 { return vec![]; }
    peaks.windows(2)
        .map(|w| (w[1] - w[0]) as f64 / sample_rate * 1000.0)
        .collect()
}

/// RMSSD — Root Mean Square of Successive Differences
fn rmssd(rr: &[f64]) -> f64 {
    if rr.len() < 2 { return 0.0; }
    let sum_sq: f64 = rr.windows(2)
        .map(|w| (w[1] - w[0]).powi(2))
        .sum();
    (sum_sq / (rr.len() - 1) as f64).sqrt()
}

/// pNN50 — percentage of successive R-R differences > 50ms
fn pnn50(rr: &[f64]) -> f64 {
    if rr.len() < 2 { return 0.0; }
    let count = rr.windows(2)
        .filter(|w| (w[1] - w[0]).abs() > 50.0)
        .count();
    count as f64 / (rr.len() - 1) as f64
}

/// Coefficient of variation of R-R intervals
fn rr_coefficient_of_variation(rr: &[f64]) -> f64 {
    if rr.is_empty() { return 0.0; }
    let mean = rr.iter().sum::<f64>() / rr.len() as f64;
    if mean < 1e-6 { return 0.0; }
    let var = rr.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / rr.len() as f64;
    var.sqrt() / mean
}

/// Mean heart rate from R-R intervals (ms)
fn mean_hr_bpm(rr: &[f64]) -> f64 {
    if rr.is_empty() { return 0.0; }
    let mean_rr = rr.iter().sum::<f64>() / rr.len() as f64;
    if mean_rr < 1.0 { return 0.0; }
    60000.0 / mean_rr
}

#[derive(serde::Serialize)]
struct CardiacReport {
    heart_rate_bpm: f64,
    rmssd_ms: f64,
    pnn50: f64,
    rr_cv: f64,
    r_peaks_detected: usize,
    rhythm: String,
    alerts: Vec<String>,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_report(report: &CardiacReport) -> Result<(), String> {
    let rhythm_code = match report.rhythm.as_str() {
        "normal_sinus" => 0.0,
        "tachycardia" => 0.3,
        "bradycardia" => 0.5,
        "atrial_fibrillation" => 0.9,
        _ => 0.1,
    };
    let vector = vec![
        report.heart_rate_bpm / 200.0,
        report.rmssd_ms / 200.0,
        report.pnn50,
        report.rr_cv,
        rhythm_code,
        if report.alerts.is_empty() { 0.0 } else { 1.0 },
        report.r_peaks_detected as f64 / 50.0,
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

fn run_once() -> Result<CardiacReport, String> {
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
    // Bandpass 0.8-2.5 Hz for heart rate
    let mut hr_filter = BandpassFilter::new(0.8, 2.5, sample_rate);
    let filtered: Vec<f64> = amplitudes.iter().map(|&v| hr_filter.process(v)).collect();

    // Detect R-peaks
    let peaks = detect_r_peaks(&filtered, 0.4);
    let rr = rr_intervals_ms(&peaks, sample_rate);

    let hr_bpm = mean_hr_bpm(&rr);
    let rmssd_val = rmssd(&rr);
    let pnn50_val = pnn50(&rr);
    let cv = rr_coefficient_of_variation(&rr);

    // Classify rhythm
    let mut alerts = Vec::new();
    let rhythm = if rr.len() < 2 {
        "insufficient_data".to_string()
    } else if cv > 0.20 {
        // High irregularity suggests AF
        alerts.push(format!("AF_SUSPECTED: R-R CV={:.3} (>0.20)", cv));
        "atrial_fibrillation".to_string()
    } else if hr_bpm > 100.0 {
        alerts.push(format!("TACHYCARDIA: HR={:.0} bpm", hr_bpm));
        "tachycardia".to_string()
    } else if hr_bpm > 0.0 && hr_bpm < 50.0 {
        alerts.push(format!("BRADYCARDIA: HR={:.0} bpm", hr_bpm));
        "bradycardia".to_string()
    } else {
        "normal_sinus".to_string()
    };

    // Additional HRV alerts
    if rmssd_val > 0.0 && rmssd_val < 20.0 && rr.len() >= 2 {
        alerts.push(format!("LOW_HRV: RMSSD={:.1}ms (autonomic stress)", rmssd_val));
    }

    Ok(CardiacReport {
        heart_rate_bpm: hr_bpm,
        rmssd_ms: rmssd_val,
        pnn50: pnn50_val,
        rr_cv: cv,
        r_peaks_detected: peaks.len(),
        rhythm,
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

    eprintln!("[cog-cardiac-arrhythmia] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_report(&report) {
                    eprintln!("[cog-cardiac-arrhythmia] store error: {e}");
                }
                if !report.alerts.is_empty() {
                    eprintln!("[cog-cardiac-arrhythmia] ALERT: {:?}", report.alerts);
                }
            }
            Err(e) => eprintln!("[cog-cardiac-arrhythmia] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
