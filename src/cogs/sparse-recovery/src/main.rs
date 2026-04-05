//! Cognitum Cog: Sparse Recovery
//!
//! Recovers missing signal data using L1-minimization proxy. If some channels
//! report 0/NaN, estimates from correlated channels using simple regression.
//!
//! Usage:
//!   cog-sparse-recovery --once
//!   cog-sparse-recovery --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 50;

struct RegressionModel {
    /// Per-channel history for building regression
    history: Vec<Vec<f64>>,
    num_channels: usize,
}

impl RegressionModel {
    fn new() -> Self { Self { history: Vec::new(), num_channels: 0 } }

    fn update(&mut self, values: &[f64]) {
        if values.len() != self.num_channels {
            self.num_channels = values.len();
            self.history = vec![Vec::new(); values.len()];
        }
        for (i, &v) in values.iter().enumerate() {
            self.history[i].push(v);
            if self.history[i].len() > MAX_HISTORY {
                self.history[i].remove(0);
            }
        }
    }

    /// Simple linear regression: predict target from source channel
    fn regress(&self, source_ch: usize, target_ch: usize) -> (f64, f64) {
        let xs = &self.history[source_ch];
        let ys = &self.history[target_ch];
        let n = xs.len().min(ys.len());
        if n < 3 { return (0.0, 0.0); }

        let x_mean = xs[..n].iter().sum::<f64>() / n as f64;
        let y_mean = ys[..n].iter().sum::<f64>() / n as f64;

        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..n {
            let dx = xs[i] - x_mean;
            num += dx * (ys[i] - y_mean);
            den += dx * dx;
        }
        let slope = if den.abs() > 1e-10 { num / den } else { 0.0 };
        let intercept = y_mean - slope * x_mean;
        (slope, intercept)
    }

    /// Find best predictor channel for a given target
    fn best_predictor(&self, target_ch: usize, current_values: &[f64]) -> Option<(usize, f64, f64)> {
        let mut best: Option<(usize, f64, f64)> = None;
        let mut best_r2: f64 = -1.0;

        for src in 0..self.num_channels {
            if src == target_ch { continue; }
            // Skip if source is also missing
            if current_values[src].abs() < 1e-10 { continue; }

            let (slope, intercept) = self.regress(src, target_ch);
            // R-squared approximation
            let ys = &self.history[target_ch];
            let xs = &self.history[src];
            let n = xs.len().min(ys.len());
            if n < 3 { continue; }

            let y_mean = ys[..n].iter().sum::<f64>() / n as f64;
            let ss_tot: f64 = ys[..n].iter().map(|y| (y - y_mean).powi(2)).sum();
            let ss_res: f64 = (0..n).map(|i| {
                let pred = slope * xs[i] + intercept;
                (ys[i] - pred).powi(2)
            }).sum();
            let r2 = if ss_tot > 1e-10 { 1.0 - ss_res / ss_tot } else { 0.0 };

            if r2 > best_r2 {
                best_r2 = r2;
                let predicted = slope * current_values[src] + intercept;
                best = Some((src, predicted, r2));
            }
        }
        best
    }

    fn depth(&self) -> usize {
        self.history.first().map(|h| h.len()).unwrap_or(0)
    }
}

#[derive(serde::Serialize)]
struct RecoveryReport {
    original_values: Vec<f64>,
    recovered_values: Vec<f64>,
    missing_channels: Vec<usize>,
    recoveries: Vec<ChannelRecovery>,
    recovery_rate: f64,
    total_channels: usize,
    history_depth: usize,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct ChannelRecovery {
    channel: usize,
    original: f64,
    recovered: f64,
    predictor_channel: usize,
    r_squared: f64,
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

fn run_once(model: &mut RegressionModel) -> Result<RecoveryReport, String> {
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

    // Detect missing channels (value == 0 or NaN-like)
    let missing: Vec<usize> = values.iter().enumerate()
        .filter(|(_, v)| v.abs() < 1e-10 || v.is_nan())
        .map(|(i, _)| i)
        .collect();

    let mut recovered = values.clone();
    let mut recoveries = Vec::new();

    // Try to recover each missing channel
    if model.depth() >= 5 {
        for &ch in &missing {
            if let Some((src, pred, r2)) = model.best_predictor(ch, &values) {
                recovered[ch] = pred;
                recoveries.push(ChannelRecovery {
                    channel: ch,
                    original: values[ch],
                    recovered: pred,
                    predictor_channel: src,
                    r_squared: r2,
                });
            }
        }
    }

    // Update model with recovered values (so future predictions benefit)
    model.update(&recovered);

    let recovery_rate = if missing.is_empty() { 1.0 } else {
        recoveries.len() as f64 / missing.len() as f64
    };

    let report = RecoveryReport {
        original_values: values,
        recovered_values: recovered.clone(),
        missing_channels: missing,
        recoveries,
        recovery_rate,
        total_channels: recovered.len(),
        history_depth: model.depth(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    // Store recovered vector
    let mut vec8 = [0.0_f64; 8];
    for (i, &v) in recovered.iter().take(8).enumerate() {
        vec8[i] = v / 100.0;
    }
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-sparse-recovery] store error: {e}");
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

    eprintln!("[cog-sparse-recovery] starting (interval={}s)", interval);

    let mut model = RegressionModel::new();

    loop {
        let start = Instant::now();
        match run_once(&mut model) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if !report.missing_channels.is_empty() {
                    eprintln!("[cog-sparse-recovery] ALERT: {} missing channels, {} recovered",
                        report.missing_channels.len(), report.recoveries.len());
                }
            }
            Err(e) => eprintln!("[cog-sparse-recovery] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
