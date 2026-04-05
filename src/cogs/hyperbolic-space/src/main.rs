//! Cognitum Cog: Hyperbolic Space
//!
//! Maps signal vectors into the Poincare ball model (hyperbolic space).
//! Computes hyperbolic distances between current and historical vectors.
//! Better captures hierarchical relationships between signal states.
//!
//! Usage:
//!   cog-hyperbolic-space --once
//!   cog-hyperbolic-space --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 100;
const CURVATURE: f64 = -1.0; // Negative curvature for hyperbolic space

/// Project a Euclidean vector into the Poincare ball (norm < 1)
fn project_to_poincare(v: &[f64]) -> Vec<f64> {
    let norm_sq: f64 = v.iter().map(|x| x * x).sum();
    let norm = norm_sq.sqrt();
    if norm < 1e-10 {
        return vec![0.0; v.len()];
    }
    // Use tanh to map to open unit ball
    let scale = norm.tanh() / norm;
    v.iter().map(|x| x * scale).collect()
}

/// Poincare ball distance: d(u,v) = arcosh(1 + 2 * ||u-v||^2 / ((1-||u||^2)(1-||v||^2)))
fn poincare_distance(u: &[f64], v: &[f64]) -> f64 {
    let n = u.len().min(v.len());
    if n == 0 { return 0.0; }

    let diff_sq: f64 = (0..n).map(|i| (u[i] - v[i]).powi(2)).sum();
    let u_sq: f64 = u.iter().take(n).map(|x| x * x).sum::<f64>().min(0.9999);
    let v_sq: f64 = v.iter().take(n).map(|x| x * x).sum::<f64>().min(0.9999);

    let denom = (1.0 - u_sq) * (1.0 - v_sq);
    if denom < 1e-10 { return 0.0; }

    let arg = 1.0 + 2.0 * diff_sq / denom;
    // arcosh(x) = ln(x + sqrt(x^2 - 1)) for x >= 1
    if arg < 1.0 { return 0.0; }
    (arg + (arg * arg - 1.0).sqrt()).ln()
}

/// Mobius addition in the Poincare ball
fn mobius_addition(u: &[f64], v: &[f64]) -> Vec<f64> {
    let n = u.len().min(v.len());
    let u_sq: f64 = u.iter().take(n).map(|x| x * x).sum::<f64>().min(0.9999);
    let v_sq: f64 = v.iter().take(n).map(|x| x * x).sum::<f64>().min(0.9999);
    let uv: f64 = (0..n).map(|i| u[i] * v[i]).sum();

    let num_coeff_u = 1.0 + 2.0 * uv + v_sq;
    let num_coeff_v = 1.0 - u_sq;
    let denom = 1.0 + 2.0 * uv + u_sq * v_sq;

    if denom.abs() < 1e-10 {
        return vec![0.0; n];
    }

    (0..n).map(|i| (num_coeff_u * u[i] + num_coeff_v * v[i]) / denom).collect()
}

#[derive(serde::Serialize)]
struct HyperbolicReport {
    poincare_point: Vec<f64>,
    poincare_norm: f64,
    distances_to_history: Vec<f64>,
    mean_distance: f64,
    min_distance: f64,
    max_distance: f64,
    centroid_distance: f64,
    curvature: f64,
    history_size: usize,
    timestamp: u64,
}

struct HyperbolicHistory {
    points: Vec<Vec<f64>>,
}

impl HyperbolicHistory {
    fn new() -> Self { Self { points: Vec::new() } }

    fn push(&mut self, point: Vec<f64>) {
        self.points.push(point);
        if self.points.len() > MAX_HISTORY {
            self.points.remove(0);
        }
    }

    /// Approximate centroid using Frechet mean (iterative)
    fn approximate_centroid(&self) -> Vec<f64> {
        if self.points.is_empty() { return vec![]; }
        let dim = self.points[0].len();
        // Simple Euclidean mean then project (approximation)
        let mut mean = vec![0.0; dim];
        for p in &self.points {
            for (i, &v) in p.iter().enumerate() {
                mean[i] += v;
            }
        }
        let n = self.points.len() as f64;
        for v in &mut mean { *v /= n; }
        project_to_poincare(&mean)
    }
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

fn run_once(history: &mut HyperbolicHistory) -> Result<HyperbolicReport, String> {
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

    // Normalize and project to Poincare ball
    let max_abs = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max).max(1e-10);
    let normalized: Vec<f64> = values.iter().map(|v| v / max_abs).collect();
    let poincare_point = project_to_poincare(&normalized);
    let poincare_norm: f64 = poincare_point.iter().map(|x| x * x).sum::<f64>().sqrt();

    // Compute distances to all historical points
    let distances: Vec<f64> = history.points.iter()
        .map(|h| poincare_distance(&poincare_point, h))
        .collect();

    let mean_dist = if distances.is_empty() { 0.0 } else {
        distances.iter().sum::<f64>() / distances.len() as f64
    };
    let min_dist = distances.iter().cloned().fold(f64::MAX, f64::min);
    let max_dist = distances.iter().cloned().fold(0.0_f64, f64::max);

    let centroid = history.approximate_centroid();
    let centroid_dist = if centroid.is_empty() { 0.0 } else {
        poincare_distance(&poincare_point, &centroid)
    };

    history.push(poincare_point.clone());

    // Pad/truncate poincare_point for output
    let mut pt_out = poincare_point.clone();
    pt_out.truncate(8);
    while pt_out.len() < 8 { pt_out.push(0.0); }

    let report = HyperbolicReport {
        poincare_point: poincare_point.iter().take(8).cloned().collect(),
        poincare_norm,
        distances_to_history: distances.iter().rev().take(10).cloned().collect(),
        mean_distance: mean_dist,
        min_distance: if min_dist == f64::MAX { 0.0 } else { min_dist },
        max_distance: max_dist,
        centroid_distance: centroid_dist,
        curvature: CURVATURE,
        history_size: history.points.len(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8: [f64; 8] = [
        pt_out[0], pt_out[1], pt_out[2], pt_out[3],
        poincare_norm,
        mean_dist / 5.0,
        centroid_dist / 5.0,
        history.points.len() as f64 / MAX_HISTORY as f64,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-hyperbolic-space] store error: {e}");
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

    eprintln!("[cog-hyperbolic-space] starting (interval={}s)", interval);

    let mut history = HyperbolicHistory::new();

    loop {
        let start = Instant::now();
        match run_once(&mut history) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.centroid_distance > 2.0 {
                    eprintln!("[cog-hyperbolic-space] ALERT: large drift from centroid d={:.2}",
                        report.centroid_distance);
                }
            }
            Err(e) => eprintln!("[cog-hyperbolic-space] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
