//! Cognitum Cog: Behavioral Profiler
//!
//! Learns normal behavior patterns over time by building a codebook of
//! typical signal signatures using online k-means (nearest-centroid update).
//! Flags anything >2.5 sigma from nearest centroid as anomalous.
//!
//! Usage:
//!   cog-behavioral-profiler --once
//!   cog-behavioral-profiler --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_CENTROIDS: usize = 32;
const FEATURE_DIM: usize = 8;
const ANOMALY_THRESHOLD: f64 = 2.5;

struct Centroid {
    values: [f64; FEATURE_DIM],
    count: u64,
    sum_sq_dist: f64, // for variance estimation
}

impl Centroid {
    fn new(values: [f64; FEATURE_DIM]) -> Self {
        Self { values, count: 1, sum_sq_dist: 0.0 }
    }

    fn distance(&self, other: &[f64; FEATURE_DIM]) -> f64 {
        self.values.iter().zip(other.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    fn update(&mut self, sample: &[f64; FEATURE_DIM]) {
        let dist = self.distance(sample);
        self.count += 1;
        let lr = 1.0 / self.count as f64;
        for i in 0..FEATURE_DIM {
            self.values[i] += lr * (sample[i] - self.values[i]);
        }
        // Online variance of distances
        let delta = dist - self.mean_dist();
        self.sum_sq_dist += delta * delta;
    }

    fn mean_dist(&self) -> f64 {
        if self.count < 2 { 0.0 }
        else { (self.sum_sq_dist / (self.count - 1) as f64).sqrt() }
    }

    fn std_dist(&self) -> f64 {
        if self.count < 3 { self.mean_dist() }
        else { (self.sum_sq_dist / (self.count - 1) as f64).sqrt() }
    }
}

struct BehavioralProfiler {
    centroids: Vec<Centroid>,
    distance_stats: WelfordStats,
    learning_count: u64,
    learning_target: u64,
    total_anomalies: u64,
    total_samples: u64,
}

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

impl BehavioralProfiler {
    fn new() -> Self {
        Self {
            centroids: Vec::new(),
            distance_stats: WelfordStats::new(),
            learning_count: 0,
            learning_target: 100,
            total_anomalies: 0,
            total_samples: 0,
        }
    }

    fn extract_features(&self, amplitudes: &[f64]) -> [f64; FEATURE_DIM] {
        let n = amplitudes.len().max(1) as f64;
        let mean = amplitudes.iter().sum::<f64>() / n;
        let variance = amplitudes.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        let min = amplitudes.iter().cloned().fold(f64::MAX, f64::min);
        let max = amplitudes.iter().cloned().fold(f64::MIN, f64::max);
        let range = max - min;

        // Skewness
        let skewness = if std_dev < 1e-10 { 0.0 } else {
            amplitudes.iter().map(|v| ((v - mean) / std_dev).powi(3)).sum::<f64>() / n
        };

        // Kurtosis
        let kurtosis = if std_dev < 1e-10 { 0.0 } else {
            amplitudes.iter().map(|v| ((v - mean) / std_dev).powi(4)).sum::<f64>() / n - 3.0
        };

        // Energy (RMS)
        let rms = (amplitudes.iter().map(|v| v * v).sum::<f64>() / n).sqrt();

        // Zero-crossing rate (sign changes)
        let zcr = if amplitudes.len() < 2 { 0.0 } else {
            let mut crossings = 0u32;
            for i in 1..amplitudes.len() {
                if (amplitudes[i] > mean) != (amplitudes[i - 1] > mean) {
                    crossings += 1;
                }
            }
            crossings as f64 / (amplitudes.len() - 1) as f64
        };

        [mean, std_dev, range, skewness, kurtosis, rms, zcr, variance]
    }

    fn process(&mut self, amplitudes: &[f64]) -> ProfileResult {
        let features = self.extract_features(amplitudes);
        self.total_samples += 1;

        // Find nearest centroid
        let (nearest_idx, nearest_dist) = if self.centroids.is_empty() {
            (None, f64::MAX)
        } else {
            let mut best_idx = 0;
            let mut best_dist = self.centroids[0].distance(&features);
            for (i, c) in self.centroids.iter().enumerate().skip(1) {
                let d = c.distance(&features);
                if d < best_dist {
                    best_dist = d;
                    best_idx = i;
                }
            }
            (Some(best_idx), best_dist)
        };

        // Learning phase: build codebook
        if self.learning_count < self.learning_target {
            self.learning_count += 1;

            if self.centroids.is_empty() || (nearest_dist > 1.0 && self.centroids.len() < MAX_CENTROIDS) {
                self.centroids.push(Centroid::new(features));
            } else if let Some(idx) = nearest_idx {
                self.centroids[idx].update(&features);
            }

            self.distance_stats.update(nearest_dist);

            return ProfileResult::Learning {
                progress_pct: (self.learning_count as f64 / self.learning_target as f64 * 100.0).min(100.0),
                codebook_size: self.centroids.len(),
            };
        }

        // Active phase: classify
        let z = self.distance_stats.z_score(nearest_dist);
        let is_anomaly = z > ANOMALY_THRESHOLD;

        if is_anomaly {
            self.total_anomalies += 1;
        } else {
            // Update nearest centroid (slow adaptation)
            if let Some(idx) = nearest_idx {
                self.centroids[idx].update(&features);
            }
            self.distance_stats.update(nearest_dist);
        }

        ProfileResult::Active {
            is_anomaly,
            nearest_distance: nearest_dist,
            z_score: z,
            nearest_centroid: nearest_idx.unwrap_or(0),
            codebook_size: self.centroids.len(),
        }
    }
}

enum ProfileResult {
    Learning { progress_pct: f64, codebook_size: usize },
    Active {
        is_anomaly: bool,
        nearest_distance: f64,
        z_score: f64,
        nearest_centroid: usize,
        codebook_size: usize,
    },
}

#[derive(serde::Serialize)]
struct ProfileReport {
    status: String,
    anomaly_detected: bool,
    nearest_distance: f64,
    z_score: f64,
    codebook_size: usize,
    total_anomalies: u64,
    total_samples: u64,
    anomaly_rate: f64,
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

fn store_vector(report: &ProfileReport) -> Result<(), String> {
    let vector = vec![
        if report.anomaly_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.nearest_distance.min(10.0) / 10.0,
        report.z_score.abs().min(10.0) / 10.0,
        report.anomaly_rate,
        report.codebook_size as f64 / MAX_CENTROIDS as f64,
        report.total_anomalies as f64 / report.total_samples.max(1) as f64,
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
        .unwrap_or(5);

    eprintln!("[cog-behavioral-profiler] starting (interval={}s)", interval);
    let mut profiler = BehavioralProfiler::new();

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

                    let result = profiler.process(&amps);
                    let report = match result {
                        ProfileResult::Learning { progress_pct, codebook_size } => ProfileReport {
                            status: format!("learning ({:.0}%, {} centroids)", progress_pct, codebook_size),
                            anomaly_detected: false,
                            nearest_distance: 0.0, z_score: 0.0,
                            codebook_size,
                            total_anomalies: 0, total_samples: profiler.total_samples,
                            anomaly_rate: 0.0, confidence: 0.0, timestamp: now_ts(),
                        },
                        ProfileResult::Active { is_anomaly, nearest_distance, z_score, nearest_centroid, codebook_size } => {
                            let confidence = if is_anomaly {
                                (z_score / (ANOMALY_THRESHOLD * 2.0)).min(1.0)
                            } else { 0.0 };
                            let anomaly_rate = profiler.total_anomalies as f64
                                / profiler.total_samples.max(1) as f64;
                            ProfileReport {
                                status: if is_anomaly { "ANOMALY".into() }
                                    else { "normal".into() },
                                anomaly_detected: is_anomaly,
                                nearest_distance, z_score,
                                codebook_size,
                                total_anomalies: profiler.total_anomalies,
                                total_samples: profiler.total_samples,
                                anomaly_rate, confidence, timestamp: now_ts(),
                            }
                        }
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-behavioral-profiler] store error: {e}");
                    }
                    if report.anomaly_detected {
                        eprintln!("[cog-behavioral-profiler] ANOMALY: z={:.2}, dist={:.4}, rate={:.2}%",
                            report.z_score, report.nearest_distance, report.anomaly_rate * 100.0);
                    }
                }
            }
            Err(e) => eprintln!("[cog-behavioral-profiler] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
