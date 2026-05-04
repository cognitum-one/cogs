//! Cognitum Cog: Panic Motion Detection
//!
//! Detects sudden erratic movement via high-frequency signal energy
//! (>3Hz) spike combined with rapid direction changes (sign changes
//! in derivative). Computes "panic score" from jerk (derivative of
//! acceleration proxy).
//!
//! Usage:
//!   cog-panic-motion --once
//!   cog-panic-motion --interval 1

use std::io::Read;
use std::time::{Duration, Instant};

struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordStats {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }
    fn std_dev(&self) -> f64 {
        if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() }
    }
    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { 0.0 } else { (value - self.mean) / sd }
    }
}

const WINDOW_SIZE: usize = 32;

struct PanicDetector {
    signal_window: Vec<f64>,
    energy_baseline: WelfordStats,
    jerk_baseline: WelfordStats,
    direction_change_baseline: WelfordStats,
    learning_samples: u64,
    panic_threshold: f64,
    total_panics: u64,
    consecutive_panic: u32,
    panic_required: u32,
    prev_derivative: Option<f64>,
    prev_value: Option<f64>,
    prev_accel: Option<f64>,
}

impl PanicDetector {
    fn new() -> Self {
        Self {
            signal_window: Vec::with_capacity(WINDOW_SIZE),
            energy_baseline: WelfordStats::new(),
            jerk_baseline: WelfordStats::new(),
            direction_change_baseline: WelfordStats::new(),
            learning_samples: 50,
            panic_threshold: 3.0,
            total_panics: 0,
            consecutive_panic: 0,
            panic_required: 2,
            prev_derivative: None,
            prev_value: None,
            prev_accel: None,
        }
    }

    fn process(&mut self, amplitudes: &[f64]) -> PanicResult {
        // Mean signal as motion proxy
        let mean = amplitudes.iter().sum::<f64>() / amplitudes.len().max(1) as f64;

        // Push into window
        self.signal_window.push(mean);
        if self.signal_window.len() > WINDOW_SIZE {
            self.signal_window.remove(0);
        }

        if self.signal_window.len() < 4 {
            return PanicResult::Collecting;
        }

        // Compute derivatives from window
        let n = self.signal_window.len();
        let mut derivatives = Vec::with_capacity(n - 1);
        for i in 1..n {
            derivatives.push(self.signal_window[i] - self.signal_window[i - 1]);
        }

        // Acceleration (2nd derivative)
        let mut accels = Vec::with_capacity(derivatives.len() - 1);
        for i in 1..derivatives.len() {
            accels.push(derivatives[i] - derivatives[i - 1]);
        }

        // Jerk (3rd derivative) — rate of change of acceleration
        let mut jerks = Vec::with_capacity(accels.len().saturating_sub(1));
        for i in 1..accels.len() {
            jerks.push(accels[i] - accels[i - 1]);
        }

        // High-frequency energy: variance of derivatives (proxy for >3Hz content)
        let hf_energy = if derivatives.is_empty() { 0.0 } else {
            let mean_d = derivatives.iter().sum::<f64>() / derivatives.len() as f64;
            derivatives.iter().map(|d| (d - mean_d).powi(2)).sum::<f64>() / derivatives.len() as f64
        };

        // Direction changes: sign changes in derivative
        let direction_changes = if derivatives.len() < 2 { 0 } else {
            let mut changes = 0u32;
            for i in 1..derivatives.len() {
                if (derivatives[i] > 0.0) != (derivatives[i - 1] > 0.0) {
                    changes += 1;
                }
            }
            changes
        };

        // Jerk magnitude
        let jerk_rms = if jerks.is_empty() { 0.0 } else {
            (jerks.iter().map(|j| j * j).sum::<f64>() / jerks.len() as f64).sqrt()
        };

        // Learning phase
        if self.energy_baseline.count < self.learning_samples {
            self.energy_baseline.update(hf_energy);
            self.jerk_baseline.update(jerk_rms);
            self.direction_change_baseline.update(direction_changes as f64);
            return PanicResult::Learning {
                progress_pct: (self.energy_baseline.count as f64 / self.learning_samples as f64 * 100.0).min(100.0),
            };
        }

        // Z-scores for each component
        let z_energy = self.energy_baseline.z_score(hf_energy);
        let z_jerk = self.jerk_baseline.z_score(jerk_rms);
        let z_dir = self.direction_change_baseline.z_score(direction_changes as f64);

        // Panic score: weighted combination
        let panic_score = z_energy.max(0.0) * 0.4
            + z_jerk.max(0.0) * 0.35
            + z_dir.max(0.0) * 0.25;

        let is_panic = panic_score > self.panic_threshold;

        if is_panic {
            self.consecutive_panic += 1;
        } else {
            self.consecutive_panic = 0;
            // Slow adaptation
            self.energy_baseline.update(hf_energy);
            self.jerk_baseline.update(jerk_rms);
            self.direction_change_baseline.update(direction_changes as f64);
        }

        let confirmed = self.consecutive_panic >= self.panic_required;
        if confirmed && self.consecutive_panic == self.panic_required {
            self.total_panics += 1;
        }

        PanicResult::Active {
            panic_detected: confirmed,
            panic_score,
            hf_energy,
            jerk_rms,
            direction_changes,
            z_energy,
            z_jerk,
            z_direction: z_dir,
        }
    }
}

enum PanicResult {
    Collecting,
    Learning { progress_pct: f64 },
    Active {
        panic_detected: bool,
        panic_score: f64,
        hf_energy: f64,
        jerk_rms: f64,
        direction_changes: u32,
        z_energy: f64,
        z_jerk: f64,
        z_direction: f64,
    },
}

#[derive(serde::Serialize)]
struct PanicReport {
    status: String,
    panic_detected: bool,
    panic_score: f64,
    hf_energy: f64,
    jerk_rms: f64,
    direction_changes: u32,
    confidence: f64,
    total_panics: u64,
    timestamp: u64,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(report: &PanicReport) -> Result<(), String> {
    let vector = vec![
        if report.panic_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.panic_score / 10.0,
        report.hf_energy.min(1.0),
        report.jerk_rms.min(1.0),
        report.direction_changes as f64 / 32.0,
        report.total_panics as f64 / 100.0,
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);

    eprintln!("[cog-panic-motion] starting (interval={}s)", interval);
    let mut detector = PanicDetector::new();

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|s| s.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();
                    if amps.is_empty() { continue; }

                    let result = detector.process(&amps);
                    let report = match result {
                        PanicResult::Collecting => PanicReport {
                            status: "collecting".into(),
                            panic_detected: false, panic_score: 0.0,
                            hf_energy: 0.0, jerk_rms: 0.0, direction_changes: 0,
                            confidence: 0.0, total_panics: 0, timestamp: now_ts(),
                        },
                        PanicResult::Learning { progress_pct } => PanicReport {
                            status: format!("learning ({:.0}%)", progress_pct),
                            panic_detected: false, panic_score: 0.0,
                            hf_energy: 0.0, jerk_rms: 0.0, direction_changes: 0,
                            confidence: 0.0, total_panics: 0, timestamp: now_ts(),
                        },
                        PanicResult::Active {
                            panic_detected, panic_score, hf_energy,
                            jerk_rms, direction_changes, ..
                        } => {
                            let confidence = if panic_detected {
                                (panic_score / (detector.panic_threshold * 2.0)).min(1.0)
                            } else { 0.0 };
                            PanicReport {
                                status: if panic_detected { "PANIC".into() }
                                    else if panic_score > 1.5 { "elevated".into() }
                                    else { "calm".into() },
                                panic_detected,
                                panic_score,
                                hf_energy,
                                jerk_rms,
                                direction_changes,
                                confidence,
                                total_panics: detector.total_panics,
                                timestamp: now_ts(),
                            }
                        }
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-panic-motion] store error: {e}");
                    }
                    if report.panic_detected {
                        eprintln!("[cog-panic-motion] ALERT: panic motion detected (score={:.2}, confidence={:.0}%)",
                            report.panic_score, report.confidence * 100.0);
                    }
                }
            }
            Err(e) => eprintln!("[cog-panic-motion] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
