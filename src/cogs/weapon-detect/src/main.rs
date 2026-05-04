//! Cognitum Cog: Weapon Detect — Concealed Metal Object Detection
//!
//! Metal objects cause consistent amplitude reduction across multiple
//! subcarriers. Detects cross-channel correlation drop as a signal
//! that a metallic object is absorbing/reflecting WiFi energy.
//!
//! Usage:
//!   cog-weapon-detect --once
//!   cog-weapon-detect --interval 1

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

const MAX_CHANNELS: usize = 64;
const HISTORY_LEN: usize = 10;

struct WeaponDetector {
    channel_baselines: Vec<WelfordStats>,
    absorption_baseline: WelfordStats,
    correlation_baseline: WelfordStats,
    history: Vec<Vec<f64>>,
    learning_samples: u64,
    threshold_absorption: f64,
    threshold_correlation: f64,
    total_detections: u64,
    consecutive_flags: u32,
    flag_threshold: u32,
}

impl WeaponDetector {
    fn new() -> Self {
        Self {
            channel_baselines: Vec::new(),
            absorption_baseline: WelfordStats::new(),
            correlation_baseline: WelfordStats::new(),
            history: Vec::new(),
            learning_samples: 50,
            threshold_absorption: 2.5,
            threshold_correlation: 2.0,
            total_detections: 0,
            consecutive_flags: 0,
            flag_threshold: 3,
        }
    }

    fn process(&mut self, amplitudes: &[f64]) -> DetectionResult {
        // Ensure baselines exist for each channel
        while self.channel_baselines.len() < amplitudes.len().min(MAX_CHANNELS) {
            self.channel_baselines.push(WelfordStats::new());
        }

        // Track per-channel z-scores for absorption pattern
        let mut z_scores = Vec::with_capacity(amplitudes.len());
        for (i, &amp) in amplitudes.iter().enumerate().take(self.channel_baselines.len()) {
            z_scores.push(self.channel_baselines[i].z_score(amp));
        }

        // Metal absorption: consistent negative z-scores across channels
        let negative_count = z_scores.iter().filter(|z| **z < -1.5).count();
        let absorption_ratio = if z_scores.is_empty() { 0.0 }
            else { negative_count as f64 / z_scores.len() as f64 };

        // Cross-channel correlation: how correlated are the amplitude changes?
        let correlation = self.compute_cross_correlation(amplitudes);

        // Store in history
        self.history.push(amplitudes.to_vec());
        if self.history.len() > HISTORY_LEN {
            self.history.remove(0);
        }

        // Learning phase
        let min_count = self.channel_baselines.iter()
            .map(|b| b.count)
            .min()
            .unwrap_or(0);
        if min_count < self.learning_samples {
            for (i, &amp) in amplitudes.iter().enumerate().take(self.channel_baselines.len()) {
                self.channel_baselines[i].update(amp);
            }
            self.absorption_baseline.update(absorption_ratio);
            self.correlation_baseline.update(correlation);
            return DetectionResult::Learning {
                progress_pct: (min_count as f64 / self.learning_samples as f64 * 100.0).min(100.0),
            };
        }

        // Detection: metal causes high absorption ratio + correlation drop
        let z_absorption = self.absorption_baseline.z_score(absorption_ratio);
        let z_correlation = self.correlation_baseline.z_score(correlation);

        // Metal objects: high absorption (positive z) + correlation disruption
        let metal_score = z_absorption.max(0.0) + (-z_correlation).max(0.0);
        let is_suspicious = z_absorption > self.threshold_absorption
            || (absorption_ratio > 0.5 && z_correlation < -self.threshold_correlation);

        if is_suspicious {
            self.consecutive_flags += 1;
        } else {
            self.consecutive_flags = 0;
            // Slow adaptation
            for (i, &amp) in amplitudes.iter().enumerate().take(self.channel_baselines.len()) {
                self.channel_baselines[i].update(amp);
            }
            self.absorption_baseline.update(absorption_ratio);
            self.correlation_baseline.update(correlation);
        }

        let detected = self.consecutive_flags >= self.flag_threshold;
        if detected && self.consecutive_flags == self.flag_threshold {
            self.total_detections += 1;
        }

        DetectionResult::Active {
            detected,
            metal_score,
            absorption_ratio,
            correlation,
            z_absorption,
            z_correlation,
            channels_affected: negative_count,
        }
    }

    fn compute_cross_correlation(&self, current: &[f64]) -> f64 {
        if self.history.is_empty() || current.len() < 2 {
            return 1.0;
        }
        let prev = &self.history[self.history.len() - 1];
        let n = current.len().min(prev.len());
        if n < 2 { return 1.0; }

        let mean_c = current.iter().take(n).sum::<f64>() / n as f64;
        let mean_p = prev.iter().take(n).sum::<f64>() / n as f64;

        let mut cov = 0.0;
        let mut var_c = 0.0;
        let mut var_p = 0.0;
        for i in 0..n {
            let dc = current[i] - mean_c;
            let dp = prev[i] - mean_p;
            cov += dc * dp;
            var_c += dc * dc;
            var_p += dp * dp;
        }

        let denom = (var_c * var_p).sqrt();
        if denom < 1e-10 { 1.0 } else { cov / denom }
    }
}

enum DetectionResult {
    Learning { progress_pct: f64 },
    Active {
        detected: bool,
        metal_score: f64,
        absorption_ratio: f64,
        correlation: f64,
        z_absorption: f64,
        z_correlation: f64,
        channels_affected: usize,
    },
}

#[derive(serde::Serialize)]
struct WeaponReport {
    status: String,
    metal_detected: bool,
    metal_score: f64,
    absorption_ratio: f64,
    cross_correlation: f64,
    channels_affected: usize,
    total_channels: usize,
    total_detections: u64,
    confidence: f64,
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

fn store_vector(report: &WeaponReport) -> Result<(), String> {
    let vector = vec![
        if report.metal_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.absorption_ratio,
        report.cross_correlation,
        report.metal_score / 10.0,
        report.channels_affected as f64 / report.total_channels.max(1) as f64,
        report.total_detections as f64 / 1000.0,
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

    eprintln!("[cog-weapon-detect] starting (interval={}s)", interval);
    let mut detector = WeaponDetector::new();

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
                        DetectionResult::Learning { progress_pct } => WeaponReport {
                            status: format!("learning ({:.0}%)", progress_pct),
                            metal_detected: false,
                            metal_score: 0.0,
                            absorption_ratio: 0.0,
                            cross_correlation: 1.0,
                            channels_affected: 0,
                            total_channels: amps.len(),
                            total_detections: 0,
                            confidence: 0.0,
                            timestamp: now_ts(),
                        },
                        DetectionResult::Active {
                            detected, metal_score, absorption_ratio,
                            correlation, z_absorption, z_correlation,
                            channels_affected,
                        } => {
                            let confidence = if detected {
                                (metal_score / 8.0).min(1.0)
                            } else { 0.0 };
                            WeaponReport {
                                status: if detected { "METAL_DETECTED".into() }
                                    else if metal_score > 2.0 { "suspicious".into() }
                                    else { "clear".into() },
                                metal_detected: detected,
                                metal_score,
                                absorption_ratio,
                                cross_correlation: correlation,
                                channels_affected,
                                total_channels: amps.len(),
                                total_detections: detector.total_detections,
                                confidence,
                                timestamp: now_ts(),
                            }
                        }
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-weapon-detect] store error: {e}");
                    }
                    if report.metal_detected {
                        eprintln!("[cog-weapon-detect] ALERT: concealed metal detected (confidence={:.0}%)",
                            report.confidence * 100.0);
                    }
                }
            }
            Err(e) => eprintln!("[cog-weapon-detect] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
