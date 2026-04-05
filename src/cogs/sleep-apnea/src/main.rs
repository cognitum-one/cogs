//! Cognitum Cog: Sleep Apnea Detector
//!
//! Detects breathing cessation during sleep by monitoring respiratory
//! signal variance. Uses CUSUM for gradual amplitude decrease detection
//! and Welford stats for sudden flatline detection.
//!
//! Clinical thresholds:
//!   - Apnea: breathing amplitude drops >80% for >=10s
//!   - Hypopnea: drops >30% for >=10s
//!   - AHI (Apnea-Hypopnea Index): events per hour
//!
//! Usage:
//!   cog-sleep-apnea --once
//!   cog-sleep-apnea --interval 5

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

struct ApneaDetector {
    baseline_amplitude: f64,
    baseline_count: u64,
    event_start: Option<Instant>,
    current_drop_pct: f64,
    apnea_events: u64,
    hypopnea_events: u64,
    session_start: Instant,
    // CUSUM for gradual decrease
    cusum_neg: f64,
    cusum_threshold: f64,
}

impl ApneaDetector {
    fn new() -> Self {
        Self {
            baseline_amplitude: 0.0,
            baseline_count: 0,
            event_start: None,
            current_drop_pct: 0.0,
            apnea_events: 0,
            hypopnea_events: 0,
            session_start: Instant::now(),
            cusum_neg: 0.0,
            cusum_threshold: 3.0,
        }
    }

    fn update(&mut self, breathing_amplitude: f64) -> ApneaStatus {
        // Build baseline from first 30 samples
        if self.baseline_count < 30 {
            self.baseline_count += 1;
            let alpha = 1.0 / self.baseline_count as f64;
            self.baseline_amplitude = self.baseline_amplitude * (1.0 - alpha) + breathing_amplitude * alpha;
            return ApneaStatus::Calibrating;
        }

        // Slow baseline adaptation (only when breathing normally)
        let drop_pct = if self.baseline_amplitude > 1e-6 {
            1.0 - (breathing_amplitude / self.baseline_amplitude)
        } else { 0.0 };

        self.current_drop_pct = drop_pct;

        // CUSUM for gradual decrease
        let deviation = (self.baseline_amplitude - breathing_amplitude).max(0.0);
        self.cusum_neg = (self.cusum_neg + deviation - 0.5).max(0.0);

        if drop_pct > 0.30 || self.cusum_neg > self.cusum_threshold {
            // Potential event
            if self.event_start.is_none() {
                self.event_start = Some(Instant::now());
            }
            let event_duration = self.event_start.unwrap().elapsed().as_secs();

            if event_duration >= 10 {
                if drop_pct > 0.80 {
                    return ApneaStatus::Apnea { duration_secs: event_duration };
                } else {
                    return ApneaStatus::Hypopnea { duration_secs: event_duration };
                }
            }
            ApneaStatus::Suspicious { drop_pct, duration_secs: event_duration }
        } else {
            // Event ended — count it if it was long enough
            if let Some(start) = self.event_start.take() {
                let dur = start.elapsed().as_secs();
                if dur >= 10 {
                    if self.current_drop_pct > 0.80 {
                        self.apnea_events += 1;
                    } else {
                        self.hypopnea_events += 1;
                    }
                }
            }
            self.cusum_neg = 0.0;
            // Adapt baseline slowly
            self.baseline_amplitude = self.baseline_amplitude * 0.99 + breathing_amplitude * 0.01;
            ApneaStatus::Normal
        }
    }

    fn ahi(&self) -> f64 {
        let hours = self.session_start.elapsed().as_secs_f64() / 3600.0;
        if hours < 0.01 { return 0.0; }
        (self.apnea_events + self.hypopnea_events) as f64 / hours
    }
}

enum ApneaStatus {
    Calibrating,
    Normal,
    Suspicious { drop_pct: f64, duration_secs: u64 },
    Hypopnea { duration_secs: u64 },
    Apnea { duration_secs: u64 },
}

#[derive(serde::Serialize)]
struct ApneaReport {
    status: String,
    severity: String,
    breathing_drop_pct: f64,
    event_duration_secs: u64,
    apnea_events: u64,
    hypopnea_events: u64,
    ahi: f64,
    ahi_severity: String,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
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

fn store_apnea(report: &ApneaReport) -> Result<(), String> {
    let vector = vec![
        report.breathing_drop_pct,
        report.event_duration_secs as f64 / 60.0,
        report.ahi / 30.0,
        match report.severity.as_str() { "apnea" => 1.0, "hypopnea" => 0.7, "suspicious" => 0.3, _ => 0.0 },
        report.apnea_events as f64 / 100.0,
        report.hypopnea_events as f64 / 100.0,
        0.0, 0.0,
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5);

    eprintln!("[cog-sleep-apnea] starting (interval={}s)", interval);
    let mut detector = ApneaDetector::new();
    let sample_rate = 10.0;
    let mut breathing_filter = BandpassFilter::new(0.1, 0.5, sample_rate);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();

                    // Extract breathing signal amplitude
                    let filtered: Vec<f64> = amps.iter().map(|&v| breathing_filter.process(v)).collect();
                    let breathing_amp = filtered.iter().map(|v| v.abs()).sum::<f64>() / filtered.len().max(1) as f64;

                    let status = detector.update(breathing_amp);
                    let (status_str, severity, event_dur) = match status {
                        ApneaStatus::Calibrating => ("calibrating", "none", 0),
                        ApneaStatus::Normal => ("normal", "none", 0),
                        ApneaStatus::Suspicious { duration_secs, .. } => ("suspicious", "suspicious", duration_secs),
                        ApneaStatus::Hypopnea { duration_secs } => ("HYPOPNEA", "hypopnea", duration_secs),
                        ApneaStatus::Apnea { duration_secs } => ("APNEA", "apnea", duration_secs),
                    };

                    let ahi = detector.ahi();
                    let ahi_severity = if ahi < 5.0 { "normal" }
                        else if ahi < 15.0 { "mild" }
                        else if ahi < 30.0 { "moderate" }
                        else { "severe" };

                    let report = ApneaReport {
                        status: status_str.into(),
                        severity: severity.into(),
                        breathing_drop_pct: detector.current_drop_pct,
                        event_duration_secs: event_dur,
                        apnea_events: detector.apnea_events,
                        hypopnea_events: detector.hypopnea_events,
                        ahi,
                        ahi_severity: ahi_severity.into(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_apnea(&report) {
                        eprintln!("[cog-sleep-apnea] store error: {e}");
                    }
                    if severity == "apnea" || severity == "hypopnea" {
                        eprintln!("[cog-sleep-apnea] ALERT: {} for {}s (AHI={:.1})", status_str, event_dur, ahi);
                    }
                }
            }
            Err(e) => eprintln!("[cog-sleep-apnea] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
