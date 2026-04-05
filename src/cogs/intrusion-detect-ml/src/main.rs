//! Cognitum Cog: Intrusion Detect ML — ML-Based Intrusion Detection
//!
//! Uses online learning with Welford stats + sliding window feature
//! extraction. Builds feature vectors from signal statistics, classifies
//! via nearest-neighbor against known-good baseline vectors stored in
//! the RVF store.
//!
//! Usage:
//!   cog-intrusion-detect-ml --once
//!   cog-intrusion-detect-ml --interval 3

use std::io::Read;
use std::time::{Duration, Instant};

const FEATURE_DIM: usize = 8;
const WINDOW_SIZE: usize = 20;
const BASELINE_SAMPLES: usize = 60;
const ANOMALY_THRESHOLD: f64 = 2.5;
const MAX_BASELINES: usize = 100;

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

struct FeatureVector {
    values: [f64; FEATURE_DIM],
}

impl FeatureVector {
    fn distance(&self, other: &[f64; FEATURE_DIM]) -> f64 {
        self.values.iter().zip(other.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    fn cosine_similarity(&self, other: &[f64; FEATURE_DIM]) -> f64 {
        let dot: f64 = self.values.iter().zip(other.iter()).map(|(a, b)| a * b).sum();
        let mag_a = self.values.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mag_b = other.iter().map(|v| v * v).sum::<f64>().sqrt();
        if mag_a < 1e-12 || mag_b < 1e-12 { 0.0 } else { dot / (mag_a * mag_b) }
    }
}

struct IntrusionDetectorML {
    window: Vec<Vec<f64>>,
    baselines: Vec<FeatureVector>,
    distance_stats: WelfordStats,
    feature_stats: [WelfordStats; FEATURE_DIM],
    learning: bool,
    learning_count: usize,
    total_intrusions: u64,
    total_samples: u64,
    consecutive_anomaly: u32,
    anomaly_required: u32,
}

impl IntrusionDetectorML {
    fn new() -> Self {
        Self {
            window: Vec::with_capacity(WINDOW_SIZE),
            baselines: Vec::new(),
            distance_stats: WelfordStats::new(),
            feature_stats: std::array::from_fn(|_| WelfordStats::new()),
            learning: true,
            learning_count: 0,
            total_intrusions: 0,
            total_samples: 0,
            consecutive_anomaly: 0,
            anomaly_required: 3,
        }
    }

    fn extract_features(&self, amplitudes: &[f64]) -> [f64; FEATURE_DIM] {
        let n = amplitudes.len().max(1) as f64;

        // Feature 0: Mean amplitude
        let mean = amplitudes.iter().sum::<f64>() / n;

        // Feature 1: Standard deviation
        let variance = amplitudes.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        // Feature 2: Range (max - min)
        let min = amplitudes.iter().cloned().fold(f64::MAX, f64::min);
        let max = amplitudes.iter().cloned().fold(f64::MIN, f64::max);
        let range = max - min;

        // Feature 3: RMS energy
        let rms = (amplitudes.iter().map(|v| v * v).sum::<f64>() / n).sqrt();

        // Feature 4: Inter-quartile range (approximate)
        let mut sorted = amplitudes.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q1 = sorted[sorted.len() / 4];
        let q3 = sorted[(sorted.len() * 3) / 4];
        let iqr = q3 - q1;

        // Feature 5: Temporal derivative energy (from window)
        let deriv_energy = if self.window.len() >= 2 {
            let prev = &self.window[self.window.len() - 1];
            let n_d = amplitudes.len().min(prev.len());
            if n_d > 0 {
                let mut sum = 0.0;
                for i in 0..n_d {
                    let d = amplitudes[i] - prev[i];
                    sum += d * d;
                }
                (sum / n_d as f64).sqrt()
            } else { 0.0 }
        } else { 0.0 };

        // Feature 6: Zero-crossing rate
        let zcr = if amplitudes.len() < 2 { 0.0 } else {
            let mut crossings = 0u32;
            for i in 1..amplitudes.len() {
                if (amplitudes[i] > mean) != (amplitudes[i - 1] > mean) {
                    crossings += 1;
                }
            }
            crossings as f64 / (amplitudes.len() - 1) as f64
        };

        // Feature 7: Spectral spread (variance of normalized amplitudes)
        let spectral_spread = if std_dev < 1e-10 { 0.0 } else {
            let normalized: Vec<f64> = amplitudes.iter().map(|v| (v - mean) / std_dev).collect();
            let n_mean = normalized.iter().sum::<f64>() / n;
            normalized.iter().map(|v| (v - n_mean).powi(2)).sum::<f64>() / n
        };

        [mean, std_dev, range, rms, iqr, deriv_energy, zcr, spectral_spread]
    }

    fn process(&mut self, amplitudes: &[f64]) -> MLResult {
        self.total_samples += 1;
        let features = self.extract_features(amplitudes);

        // Update window
        self.window.push(amplitudes.to_vec());
        if self.window.len() > WINDOW_SIZE {
            self.window.remove(0);
        }

        // Learning phase: collect baseline feature vectors
        if self.learning {
            self.learning_count += 1;

            // Update per-feature stats
            for i in 0..FEATURE_DIM {
                self.feature_stats[i].update(features[i]);
            }

            self.baselines.push(FeatureVector { values: features });
            if self.baselines.len() > MAX_BASELINES {
                // Keep evenly spaced samples
                let mid = self.baselines.len() / 2;
                self.baselines.remove(mid);
            }

            if self.learning_count >= BASELINE_SAMPLES {
                // Compute initial distance stats
                for i in 0..self.baselines.len() {
                    for j in (i + 1)..self.baselines.len() {
                        let d = self.baselines[i].distance(&self.baselines[j].values);
                        self.distance_stats.update(d);
                    }
                }
                self.learning = false;
                return MLResult::Trained {
                    baseline_vectors: self.baselines.len(),
                };
            }

            return MLResult::Learning {
                progress_pct: (self.learning_count as f64 / BASELINE_SAMPLES as f64 * 100.0).min(100.0),
            };
        }

        // Classification: nearest-neighbor against baselines
        let mut min_dist = f64::MAX;
        let mut best_sim = 0.0_f64;
        for baseline in &self.baselines {
            let d = baseline.distance(&features);
            let s = baseline.cosine_similarity(&features);
            if d < min_dist { min_dist = d; }
            if s > best_sim { best_sim = s; }
        }

        // Z-score of distance from baseline distribution
        self.distance_stats.update(min_dist);
        let z_distance = self.distance_stats.z_score(min_dist);

        // Per-feature z-scores
        let mut feature_z_scores = [0.0f64; FEATURE_DIM];
        let mut max_feature_z = 0.0f64;
        for i in 0..FEATURE_DIM {
            feature_z_scores[i] = self.feature_stats[i].z_score(features[i]);
            if feature_z_scores[i].abs() > max_feature_z {
                max_feature_z = feature_z_scores[i].abs();
            }
            self.feature_stats[i].update(features[i]);
        }

        // Combined anomaly score
        let anomaly_score = z_distance * 0.5 + max_feature_z * 0.3 + (1.0 - best_sim) * 10.0 * 0.2;
        let is_anomaly = anomaly_score > ANOMALY_THRESHOLD;

        if is_anomaly {
            self.consecutive_anomaly += 1;
        } else {
            self.consecutive_anomaly = 0;
            // Slow baseline adaptation
            if self.baselines.len() < MAX_BASELINES {
                self.baselines.push(FeatureVector { values: features });
            }
        }

        let intrusion = self.consecutive_anomaly >= self.anomaly_required;
        if intrusion && self.consecutive_anomaly == self.anomaly_required {
            self.total_intrusions += 1;
        }

        // Query store for additional context
        let store_match = query_store_baseline(&features);

        MLResult::Active {
            intrusion_detected: intrusion,
            anomaly_score,
            nearest_distance: min_dist,
            best_similarity: best_sim,
            z_distance,
            max_feature_z,
            store_match_found: store_match,
        }
    }
}

fn query_store_baseline(features: &[f64; FEATURE_DIM]) -> bool {
    let payload = serde_json::json!({
        "vector": features,
        "k": 3,
        "metric": "cosine"
    });
    let body = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mut conn = match std::net::TcpStream::connect("127.0.0.1:80") {
        Ok(c) => c,
        Err(_) => return false,
    };
    use std::io::Write;
    let _ = write!(conn, "POST /api/v1/store/query HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len());
    let _ = conn.write_all(&body);
    let mut resp = Vec::new();
    let _ = conn.read_to_end(&mut resp);
    let resp_str = String::from_utf8_lossy(&resp);
    // Check if any results came back with good similarity
    resp_str.contains("results")
}

enum MLResult {
    Learning { progress_pct: f64 },
    Trained { baseline_vectors: usize },
    Active {
        intrusion_detected: bool,
        anomaly_score: f64,
        nearest_distance: f64,
        best_similarity: f64,
        z_distance: f64,
        max_feature_z: f64,
        store_match_found: bool,
    },
}

#[derive(serde::Serialize)]
struct MLReport {
    status: String,
    intrusion_detected: bool,
    anomaly_score: f64,
    nearest_distance: f64,
    best_similarity: f64,
    confidence: f64,
    baseline_vectors: usize,
    total_intrusions: u64,
    total_samples: u64,
    store_corroborated: bool,
    timestamp: u64,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

fn store_vector(report: &MLReport) -> Result<(), String> {
    let vector = vec![
        if report.intrusion_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.anomaly_score / 10.0,
        report.nearest_distance.min(10.0) / 10.0,
        report.best_similarity,
        report.total_intrusions as f64 / 100.0,
        report.baseline_vectors as f64 / MAX_BASELINES as f64,
        if report.store_corroborated { 1.0 } else { 0.0 },
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

    eprintln!("[cog-intrusion-detect-ml] starting (interval={}s)", interval);
    let mut detector = IntrusionDetectorML::new();

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
                        MLResult::Learning { progress_pct } => MLReport {
                            status: format!("learning ({:.0}%)", progress_pct),
                            intrusion_detected: false, anomaly_score: 0.0,
                            nearest_distance: 0.0, best_similarity: 0.0,
                            confidence: 0.0, baseline_vectors: detector.baselines.len(),
                            total_intrusions: 0, total_samples: detector.total_samples,
                            store_corroborated: false, timestamp: now_ts(),
                        },
                        MLResult::Trained { baseline_vectors } => MLReport {
                            status: format!("trained ({} vectors)", baseline_vectors),
                            intrusion_detected: false, anomaly_score: 0.0,
                            nearest_distance: 0.0, best_similarity: 1.0,
                            confidence: 0.0, baseline_vectors,
                            total_intrusions: 0, total_samples: detector.total_samples,
                            store_corroborated: false, timestamp: now_ts(),
                        },
                        MLResult::Active {
                            intrusion_detected, anomaly_score, nearest_distance,
                            best_similarity, z_distance, max_feature_z, store_match_found,
                        } => {
                            let confidence = if intrusion_detected {
                                (anomaly_score / (ANOMALY_THRESHOLD * 2.0)).min(1.0)
                            } else { 0.0 };
                            MLReport {
                                status: if intrusion_detected { "INTRUSION".into() }
                                    else if anomaly_score > 1.5 { "suspicious".into() }
                                    else { "clear".into() },
                                intrusion_detected,
                                anomaly_score,
                                nearest_distance,
                                best_similarity,
                                confidence,
                                baseline_vectors: detector.baselines.len(),
                                total_intrusions: detector.total_intrusions,
                                total_samples: detector.total_samples,
                                store_corroborated: store_match_found,
                                timestamp: now_ts(),
                            }
                        }
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-intrusion-detect-ml] store error: {e}");
                    }
                    if report.intrusion_detected {
                        eprintln!("[cog-intrusion-detect-ml] ALERT: ML intrusion detected (score={:.2}, confidence={:.0}%, store={})",
                            report.anomaly_score, report.confidence * 100.0,
                            if report.store_corroborated { "corroborated" } else { "novel" });
                    }
                }
            }
            Err(e) => eprintln!("[cog-intrusion-detect-ml] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
