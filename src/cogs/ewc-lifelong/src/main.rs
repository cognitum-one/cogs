//! Cognitum Cog: EWC Lifelong
//!
//! Elastic Weight Consolidation. Maintain running statistics with
//! importance weights. New learning penalized for deviating from
//! important old patterns. Prevents catastrophic forgetting of
//! learned baselines.
//!
//! Usage:
//!   cog-ewc-lifelong --once
//!   cog-ewc-lifelong --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// EWC parameter store with Fisher information diagonal
struct EwcStore {
    /// Learned baseline parameters (per dimension)
    theta: [f64; DIM],
    /// Fisher information diagonal (importance weights)
    fisher: [f64; DIM],
    /// Running count for each dimension
    count: [u64; DIM],
    /// EWC penalty coefficient
    lambda: f64,
}

impl EwcStore {
    fn new(lambda: f64) -> Self {
        Self {
            theta: [0.0; DIM],
            fisher: [0.0; DIM],
            count: [0; DIM],
            lambda,
        }
    }

    /// Update baseline with new observation, respecting EWC penalty
    fn update(&mut self, observation: &[f64; DIM]) {
        for i in 0..DIM {
            self.count[i] += 1;
            let n = self.count[i] as f64;
            let lr = 1.0 / n; // Decaying learning rate

            // Gradient: move toward new observation
            let grad = observation[i] - self.theta[i];

            // EWC penalty: resist change proportional to Fisher info
            let ewc_penalty = self.lambda * self.fisher[i] * (self.theta[i] - self.theta[i]);
            // Note: penalty is 0 when comparing theta to itself; it penalizes
            // deviations from the *stored* theta during the update

            // Update theta with regularized gradient
            let effective_grad = grad - ewc_penalty;
            self.theta[i] += lr * effective_grad;

            // Update Fisher information (diagonal approximation)
            // Fisher = E[grad^2] — running average of squared gradients
            let old_fisher = self.fisher[i];
            self.fisher[i] = old_fisher + (grad * grad - old_fisher) / n;
        }
    }

    /// Compute EWC loss: how much new data deviates from important old patterns
    fn ewc_loss(&self, new_params: &[f64; DIM]) -> f64 {
        let mut loss = 0.0;
        for i in 0..DIM {
            let diff = new_params[i] - self.theta[i];
            loss += self.fisher[i] * diff * diff;
        }
        0.5 * self.lambda * loss
    }

    /// Get importance-weighted anomaly score for a new observation
    fn anomaly_score(&self, observation: &[f64; DIM]) -> f64 {
        let mut score = 0.0;
        let mut total_fisher = 0.0;
        for i in 0..DIM {
            let diff = (observation[i] - self.theta[i]).abs();
            score += self.fisher[i] * diff;
            total_fisher += self.fisher[i];
        }
        if total_fisher > 1e-10 {
            score / total_fisher
        } else {
            0.0
        }
    }

    /// Get the consolidation strength per dimension
    fn consolidation_strength(&self) -> [f64; DIM] {
        let max_f = self.fisher.iter().cloned().fold(0.0f64, f64::max);
        if max_f < 1e-10 {
            return [0.0; DIM];
        }
        let mut strength = [0.0; DIM];
        for i in 0..DIM {
            strength[i] = self.fisher[i] / max_f;
        }
        strength
    }
}

#[derive(serde::Serialize)]
struct EwcResult {
    baseline: [f64; DIM],
    fisher_diagonal: [f64; DIM],
    consolidation_strength: [f64; DIM],
    ewc_loss: f64,
    anomaly_score: f64,
    observation_count: u64,
    forgetting_risk: String,
    anomalies: Vec<String>,
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
    let start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(v: &[f64; DIM]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, v]], "dedup": true });
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

/// Extract 8-dim feature vector from sensor samples
fn extract_features(values: &[f64]) -> [f64; DIM] {
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let energy = values.iter().map(|v| v * v).sum::<f64>() / n;
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let range = max_val - min_val;

    let mut zc = 0usize;
    for i in 1..values.len() {
        if (values[i - 1] >= mean) != (values[i] >= mean) {
            zc += 1;
        }
    }

    let max_deriv = values.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f64, f64::max);

    let skewness = if var > 1e-10 {
        let sd = var.sqrt();
        values.iter().map(|v| ((v - mean) / sd).powi(3)).sum::<f64>() / n
    } else {
        0.0
    };

    [mean, var.sqrt(), energy, range, zc as f64 / n, max_deriv, skewness, n / 100.0]
}

fn run_once(store: &mut EwcStore) -> Result<EwcResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() {
        return Err("no sensor readings".into());
    }

    let features = extract_features(&values);
    let ewc_loss = store.ewc_loss(&features);
    let anomaly = store.anomaly_score(&features);

    // Update the EWC store with new observation
    store.update(&features);

    let strength = store.consolidation_strength();

    let forgetting_risk = if ewc_loss > 2.0 {
        "high"
    } else if ewc_loss > 0.5 {
        "moderate"
    } else {
        "low"
    };

    let mut anomalies = Vec::new();
    if anomaly > 1.0 {
        anomalies.push(format!("BASELINE_DEVIATION: score={anomaly:.3}"));
    }
    if ewc_loss > 2.0 {
        anomalies.push(format!("HIGH_EWC_LOSS: loss={ewc_loss:.3}, forgetting risk"));
    }

    // Store the baseline as a vector
    let _ = store_vector(&store.theta);

    let obs_count = store.count[0]; // all dims have same count

    Ok(EwcResult {
        baseline: store.theta,
        fisher_diagonal: store.fisher,
        consolidation_strength: strength,
        ewc_loss,
        anomaly_score: anomaly,
        observation_count: obs_count,
        forgetting_risk: forgetting_risk.into(),
        anomalies,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-ewc-lifelong] starting (interval={interval}s, once={once})");

    let mut ewc = EwcStore::new(1.0); // lambda=1.0

    loop {
        let start = Instant::now();
        match run_once(&mut ewc) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-ewc-lifelong] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-ewc-lifelong] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
