//! Cognitum Cog: Seizure Detection
//!
//! Monitors for seizure-like patterns: sudden high-frequency burst (>5 Hz
//! content increase >3x baseline), followed by post-ictal suppression
//! (amplitude drop >60%). Uses sliding window energy ratio.
//!
//! Usage:
//!   cog-seizure-detect --once
//!   cog-seizure-detect --interval 3

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

/// Compute signal energy (mean squared amplitude) over a window
fn window_energy(signal: &[f64]) -> f64 {
    if signal.is_empty() { return 0.0; }
    signal.iter().map(|v| v * v).sum::<f64>() / signal.len() as f64
}

/// Sliding window energy ratio detector.
/// Compares short-term energy to long-term baseline.
struct EnergyRatioDetector {
    /// Long-term baseline energy (exponential moving average)
    baseline_energy: f64,
    /// Count of updates for warm-up
    count: u64,
    /// State machine
    state: SeizureState,
    /// When ictal phase started
    ictal_start: Option<Instant>,
    /// Total events detected
    total_events: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum SeizureState {
    Normal,
    Ictal,       // Active seizure (high-frequency burst)
    PostIctal,   // Suppression phase after seizure
}

impl EnergyRatioDetector {
    fn new() -> Self {
        Self {
            baseline_energy: 0.0,
            count: 0,
            state: SeizureState::Normal,
            ictal_start: None,
            total_events: 0,
        }
    }

    fn update(&mut self, hf_energy: f64, total_energy: f64) -> SeizurePhase {
        self.count += 1;

        // Warm-up: build baseline from first 20 readings
        if self.count <= 20 {
            let alpha = 1.0 / self.count as f64;
            self.baseline_energy = self.baseline_energy * (1.0 - alpha) + hf_energy * alpha;
            return SeizurePhase::Calibrating;
        }

        let energy_ratio = if self.baseline_energy > 1e-15 {
            hf_energy / self.baseline_energy
        } else {
            0.0
        };

        // Amplitude drop for post-ictal detection
        let amp_ratio = if self.baseline_energy > 1e-15 {
            total_energy / self.baseline_energy
        } else {
            1.0
        };

        match self.state {
            SeizureState::Normal => {
                if energy_ratio > 3.0 {
                    // High-frequency burst detected
                    self.state = SeizureState::Ictal;
                    self.ictal_start = Some(Instant::now());
                    self.total_events += 1;
                    SeizurePhase::IctalOnset { energy_ratio }
                } else {
                    // Slowly adapt baseline
                    self.baseline_energy = self.baseline_energy * 0.98 + hf_energy * 0.02;
                    SeizurePhase::Normal { energy_ratio }
                }
            }
            SeizureState::Ictal => {
                let duration = self.ictal_start.map_or(0, |s| s.elapsed().as_secs());
                if energy_ratio < 1.5 {
                    // Burst subsided — check for post-ictal suppression
                    if amp_ratio < 0.4 {
                        self.state = SeizureState::PostIctal;
                        SeizurePhase::PostIctalSuppression {
                            ictal_duration_secs: duration,
                            amplitude_drop_pct: (1.0 - amp_ratio) * 100.0,
                        }
                    } else {
                        self.state = SeizureState::Normal;
                        self.baseline_energy = self.baseline_energy * 0.9 + hf_energy * 0.1;
                        SeizurePhase::Normal { energy_ratio }
                    }
                } else {
                    SeizurePhase::IctalActive {
                        energy_ratio,
                        duration_secs: duration,
                    }
                }
            }
            SeizureState::PostIctal => {
                if amp_ratio > 0.7 {
                    // Recovery
                    self.state = SeizureState::Normal;
                    self.baseline_energy = self.baseline_energy * 0.8 + hf_energy * 0.2;
                    SeizurePhase::Recovery
                } else {
                    SeizurePhase::PostIctalSuppression {
                        ictal_duration_secs: self.ictal_start.map_or(0, |s| s.elapsed().as_secs()),
                        amplitude_drop_pct: (1.0 - amp_ratio) * 100.0,
                    }
                }
            }
        }
    }
}

enum SeizurePhase {
    Calibrating,
    Normal { energy_ratio: f64 },
    IctalOnset { energy_ratio: f64 },
    IctalActive { energy_ratio: f64, duration_secs: u64 },
    PostIctalSuppression { ictal_duration_secs: u64, amplitude_drop_pct: f64 },
    Recovery,
}

#[derive(serde::Serialize)]
struct SeizureReport {
    phase: String,
    hf_energy_ratio: f64,
    ictal_duration_secs: u64,
    amplitude_drop_pct: f64,
    total_events: u64,
    alerts: Vec<String>,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_report(report: &SeizureReport) -> Result<(), String> {
    let phase_code = match report.phase.as_str() {
        "normal" => 0.0, "ictal_onset" => 0.8, "ictal_active" => 1.0,
        "post_ictal" => 0.6, "recovery" => 0.2, _ => 0.1,
    };
    let vector = vec![
        phase_code,
        (report.hf_energy_ratio / 10.0).min(1.0),
        report.ictal_duration_secs as f64 / 120.0,
        report.amplitude_drop_pct / 100.0,
        report.total_events as f64 / 20.0,
        if report.alerts.is_empty() { 0.0 } else { 1.0 },
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
        .unwrap_or(3);

    eprintln!("[cog-seizure-detect] starting (interval={}s)", interval);

    let sample_rate = 10.0;
    let mut detector = EnergyRatioDetector::new();
    // High-pass filter for >3Hz content (proxy for >5Hz with limited sample rate)
    let mut hf_filter = BandpassFilter::new(3.0, 4.5, sample_rate);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();

                    // High-frequency energy
                    let hf_signal: Vec<f64> = amps.iter().map(|&v| hf_filter.process(v)).collect();
                    let hf_energy = window_energy(&hf_signal);
                    let total_energy = window_energy(&amps);

                    let phase = detector.update(hf_energy, total_energy);
                    let (phase_str, ratio, ictal_dur, amp_drop) = match phase {
                        SeizurePhase::Calibrating => ("calibrating", 0.0, 0, 0.0),
                        SeizurePhase::Normal { energy_ratio } => ("normal", energy_ratio, 0, 0.0),
                        SeizurePhase::IctalOnset { energy_ratio } => ("ictal_onset", energy_ratio, 0, 0.0),
                        SeizurePhase::IctalActive { energy_ratio, duration_secs } => ("ictal_active", energy_ratio, duration_secs, 0.0),
                        SeizurePhase::PostIctalSuppression { ictal_duration_secs, amplitude_drop_pct } => ("post_ictal", 0.0, ictal_duration_secs, amplitude_drop_pct),
                        SeizurePhase::Recovery => ("recovery", 0.0, 0, 0.0),
                    };

                    let mut alerts = Vec::new();
                    if phase_str == "ictal_onset" || phase_str == "ictal_active" {
                        alerts.push(format!("SEIZURE_DETECTED: phase={}, energy_ratio={:.1}x", phase_str, ratio));
                    }
                    if phase_str == "post_ictal" {
                        alerts.push(format!("POST_ICTAL: amplitude_drop={:.0}%, duration={}s", amp_drop, ictal_dur));
                    }

                    let report = SeizureReport {
                        phase: phase_str.into(),
                        hf_energy_ratio: ratio,
                        ictal_duration_secs: ictal_dur,
                        amplitude_drop_pct: amp_drop,
                        total_events: detector.total_events,
                        alerts: alerts.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_report(&report) {
                        eprintln!("[cog-seizure-detect] store error: {e}");
                    }
                    if !alerts.is_empty() {
                        eprintln!("[cog-seizure-detect] ALERT: {:?}", alerts);
                    }
                }
            }
            Err(e) => eprintln!("[cog-seizure-detect] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
