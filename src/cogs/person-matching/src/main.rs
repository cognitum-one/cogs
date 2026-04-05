//! Cognitum Cog: Person Matching
//!
//! Distinguishes multiple people from signal patterns. Clusters signal
//! features into distinct "person profiles" using online k-means.
//! Reports count and confidence per profile.
//!
//! Usage:
//!   cog-person-matching --once
//!   cog-person-matching --interval 10
//!   cog-person-matching --max-people 5

use std::io::Read;
use std::time::{Duration, Instant};

const FEATURE_DIM: usize = 4; // Features extracted per observation

struct PersonCluster {
    centroids: Vec<[f64; FEATURE_DIM]>,
    counts: Vec<u64>,
    max_clusters: usize,
    merge_threshold: f64,
}

impl PersonCluster {
    fn new(max_clusters: usize) -> Self {
        Self {
            centroids: Vec::new(),
            counts: Vec::new(),
            max_clusters,
            merge_threshold: 0.3,
        }
    }

    fn euclidean(a: &[f64; FEATURE_DIM], b: &[f64; FEATURE_DIM]) -> f64 {
        a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt()
    }

    /// Assign feature vector to nearest cluster or create new one
    fn assign(&mut self, features: &[f64; FEATURE_DIM]) -> (usize, f64) {
        if self.centroids.is_empty() {
            self.centroids.push(*features);
            self.counts.push(1);
            return (0, 0.0);
        }

        // Find nearest centroid
        let mut best_idx = 0;
        let mut best_dist = f64::MAX;
        for (i, c) in self.centroids.iter().enumerate() {
            let d = Self::euclidean(c, features);
            if d < best_dist {
                best_dist = d;
                best_idx = i;
            }
        }

        // If close enough, update centroid (online mean)
        if best_dist < self.merge_threshold || self.centroids.len() >= self.max_clusters {
            let n = self.counts[best_idx] as f64;
            for (i, f) in features.iter().enumerate() {
                self.centroids[best_idx][i] = (self.centroids[best_idx][i] * n + f) / (n + 1.0);
            }
            self.counts[best_idx] += 1;
            (best_idx, best_dist)
        } else {
            // Create new cluster
            let idx = self.centroids.len();
            self.centroids.push(*features);
            self.counts.push(1);
            (idx, best_dist)
        }
    }

    fn num_people(&self) -> usize {
        self.centroids.len()
    }

    /// Confidence per cluster: based on observation count
    fn confidences(&self) -> Vec<f64> {
        let total: u64 = self.counts.iter().sum();
        if total == 0 { return vec![]; }
        self.counts.iter().map(|&c| c as f64 / total as f64).collect()
    }
}

/// Extract features from a signal snapshot
fn extract_features(values: &[f64]) -> [f64; FEATURE_DIM] {
    let n = values.len() as f64;
    if n < 1.0 { return [0.0; FEATURE_DIM]; }

    let mean = values.iter().sum::<f64>() / n;
    let std_dev = (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt();

    // High-frequency energy
    let hf = if values.len() >= 2 {
        values.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f64>() / (n - 1.0)
    } else { 0.0 };

    // Skewness
    let skew = if std_dev > 1e-10 && n > 2.0 {
        values.iter().map(|v| ((v - mean) / std_dev).powi(3)).sum::<f64>() / n
    } else { 0.0 };

    // Normalize features to ~[0, 1] range
    [
        (mean / 100.0).tanh(),
        (std_dev / 50.0).tanh(),
        (hf / 100.0).tanh(),
        (skew / 2.0).tanh(),
    ]
}

#[derive(serde::Serialize)]
struct PersonReport {
    num_people: usize,
    current_person: usize,
    distance_to_match: f64,
    profiles: Vec<PersonProfile>,
    features: [f64; FEATURE_DIM],
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct PersonProfile {
    id: usize,
    observations: u64,
    confidence: f64,
    centroid: [f64; FEATURE_DIM],
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
    let json_start = body.find('{').ok_or("no JSON in response")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(vec8: [f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec8]], "dedup": true });
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

fn run_once(cluster: &mut PersonCluster) -> Result<PersonReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples in sensor data")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    if values.is_empty() {
        return Err("no sensor readings".into());
    }

    let features = extract_features(&values);
    let (person_id, distance) = cluster.assign(&features);
    let confidences = cluster.confidences();

    let profiles: Vec<PersonProfile> = cluster.centroids.iter().enumerate()
        .map(|(i, c)| PersonProfile {
            id: i,
            observations: cluster.counts[i],
            confidence: confidences.get(i).copied().unwrap_or(0.0),
            centroid: *c,
        })
        .collect();

    let report = PersonReport {
        num_people: cluster.num_people(),
        current_person: person_id,
        distance_to_match: distance,
        profiles,
        features,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        cluster.num_people() as f64 / 10.0,
        person_id as f64 / 10.0,
        distance,
        features[0], features[1], features[2], features[3],
        confidences.get(person_id).copied().unwrap_or(0.0),
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-person-matching] store error: {e}");
    }

    Ok(report)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);
    let max_people = args.iter()
        .position(|a| a == "--max-people")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(5);

    eprintln!("[cog-person-matching] starting (interval={}s, max_people={})", interval, max_people);

    let mut cluster = PersonCluster::new(max_people);

    loop {
        let start = Instant::now();
        match run_once(&mut cluster) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.num_people > 1 {
                    eprintln!("[cog-person-matching] ALERT: {} distinct people detected",
                        report.num_people);
                }
            }
            Err(e) => eprintln!("[cog-person-matching] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
