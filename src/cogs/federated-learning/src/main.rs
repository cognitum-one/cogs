//! Cognitum Cog: Federated Learning
//!
//! Cross-seed model averaging. Query peer seeds for their model parameters
//! (stored as vectors). Average with local parameters. Weighted by sample count.
//!
//! Usage:
//!   cog-federated-learning --once
//!   cog-federated-learning --interval 30

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Local model: simple online learned parameters
struct LocalModel {
    params: [f64; DIM],
    sample_count: u64,
    /// Running mean per dimension
    mean: [f64; DIM],
    /// Running M2 per dimension (for variance)
    m2: [f64; DIM],
}

impl LocalModel {
    fn new() -> Self {
        Self {
            params: [0.0; DIM],
            sample_count: 0,
            mean: [0.0; DIM],
            m2: [0.0; DIM],
        }
    }

    fn update(&mut self, features: &[f64; DIM]) {
        self.sample_count += 1;
        let n = self.sample_count as f64;
        for i in 0..DIM {
            let delta = features[i] - self.mean[i];
            self.mean[i] += delta / n;
            let delta2 = features[i] - self.mean[i];
            self.m2[i] += delta * delta2;
        }
        // Parameters are the running mean (simple model)
        self.params = self.mean;
    }

    fn variance(&self) -> [f64; DIM] {
        let mut var = [0.0; DIM];
        if self.sample_count > 1 {
            for i in 0..DIM {
                var[i] = self.m2[i] / (self.sample_count - 1) as f64;
            }
        }
        var
    }
}

/// Federated averaging: weighted average of model parameters
fn federated_average(
    local: &[f64; DIM],
    local_count: u64,
    peers: &[([f64; DIM], u64)],
) -> [f64; DIM] {
    let mut result = [0.0; DIM];
    let total_weight: u64 = local_count + peers.iter().map(|(_, c)| *c).sum::<u64>();
    if total_weight == 0 {
        return *local;
    }

    let w = local_count as f64 / total_weight as f64;
    for i in 0..DIM {
        result[i] += local[i] * w;
    }

    for (params, count) in peers {
        let w = *count as f64 / total_weight as f64;
        for i in 0..DIM {
            result[i] += params[i] * w;
        }
    }

    result
}

/// Compute model divergence (L2 distance between local and federated)
fn model_divergence(a: &[f64; DIM], b: &[f64; DIM]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt()
}

#[derive(serde::Serialize)]
struct FederatedResult {
    local_params: [f64; DIM],
    federated_params: [f64; DIM],
    local_sample_count: u64,
    peer_count: usize,
    model_divergence: f64,
    convergence_status: String,
    variance: [f64; DIM],
    vector: [f64; DIM],
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn query_store(vector: &[f64; DIM]) -> Result<Vec<Vec<f64>>, String> {
    let payload = serde_json::json!({ "vector": vector, "k": 10, "metric": "cosine" });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/query HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    let text = String::from_utf8_lossy(&resp);
    let json_start = text.find('{').or_else(|| text.find('[')).unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&text[json_start..]).unwrap_or(serde_json::json!({"results":[]}));
    let results = parsed.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    Ok(results.iter().filter_map(|r| {
        r.get("vector").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|x| x.as_f64()).collect())
    }).filter(|v: &Vec<f64>| v.len() == DIM).collect())
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

fn extract_features(values: &[f64]) -> [f64; DIM] {
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let energy = values.iter().map(|v| v * v).sum::<f64>() / n;
    let max_v = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_v = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut zc = 0;
    for i in 1..values.len() { if (values[i-1] >= mean) != (values[i] >= mean) { zc += 1; } }
    let max_d = values.windows(2).map(|w| (w[1]-w[0]).abs()).fold(0.0f64, f64::max);
    let skew = if var > 1e-10 {
        values.iter().map(|v| ((v - mean)/var.sqrt()).powi(3)).sum::<f64>() / n
    } else { 0.0 };
    [mean, var.sqrt(), energy, max_v - min_v, zc as f64 / n, max_d, skew, n / 100.0]
}

fn run_once(model: &mut LocalModel) -> Result<FederatedResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    let features = extract_features(&values);
    model.update(&features);

    // Query store for "peer" models (other stored vectors as proxy)
    let peer_vecs = query_store(&model.params).unwrap_or_default();
    let peers: Vec<([f64; DIM], u64)> = peer_vecs.iter().map(|v| {
        let mut arr = [0.0; DIM];
        for (i, &x) in v.iter().enumerate().take(DIM) { arr[i] = x; }
        (arr, 10u64) // Assume equal peer weights
    }).collect();

    let federated = federated_average(&model.params, model.sample_count, &peers);
    let divergence = model_divergence(&model.params, &federated);

    let convergence = if divergence < 0.01 {
        "converged"
    } else if divergence < 0.1 {
        "converging"
    } else {
        "divergent"
    };

    // Apply federated update (partial blend)
    let blend = 0.3; // 30% federated, 70% local
    for i in 0..DIM {
        model.params[i] = model.params[i] * (1.0 - blend) + federated[i] * blend;
    }

    let _ = store_vector(&model.params);

    Ok(FederatedResult {
        local_params: model.params,
        federated_params: federated,
        local_sample_count: model.sample_count,
        peer_count: peers.len(),
        model_divergence: divergence,
        convergence_status: convergence.into(),
        variance: model.variance(),
        vector: model.params,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    eprintln!("[cog-federated-learning] starting (interval={interval}s, once={once})");

    let mut model = LocalModel::new();

    loop {
        let start = Instant::now();
        match run_once(&mut model) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if r.model_divergence > 0.5 {
                    eprintln!("[cog-federated-learning] ALERT: high model divergence ({:.3})", r.model_divergence);
                }
            }
            Err(e) => eprintln!("[cog-federated-learning] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
