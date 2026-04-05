//! Cognitum Cog: Ghost Hunter
//!
//! Detects unexplained environmental anomalies by monitoring multi-channel
//! correlation breakdown. Normally correlated channels becoming uncorrelated
//! signals an anomaly. Reports anomaly score and affected channels.
//!
//! Usage:
//!   cog-ghost-hunter --once
//!   cog-ghost-hunter --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const BASELINE_WINDOW: usize = 50;

struct CorrelationTracker {
    /// Rolling baseline correlations between channel pairs
    baseline: Vec<Vec<f64>>,
    /// History of per-channel values for baseline
    history: Vec<Vec<f64>>,
    num_channels: usize,
}

impl CorrelationTracker {
    fn new() -> Self {
        Self { baseline: Vec::new(), history: Vec::new(), num_channels: 0 }
    }

    fn update(&mut self, values: &[f64]) {
        let n = values.len();
        if n != self.num_channels || n == 0 {
            self.num_channels = n;
            self.baseline = vec![vec![0.0; n]; n];
            self.history = vec![Vec::new(); n];
        }
        for (i, &v) in values.iter().enumerate() {
            self.history[i].push(v);
            if self.history[i].len() > BASELINE_WINDOW {
                self.history[i].remove(0);
            }
        }
    }

    /// Compute current pairwise Pearson correlations
    fn current_correlations(&self) -> Vec<Vec<f64>> {
        let n = self.num_channels;
        let mut corr = vec![vec![0.0; n]; n];
        if self.history.is_empty() || self.history[0].len() < 3 {
            return corr;
        }
        for i in 0..n {
            for j in (i + 1)..n {
                let r = pearson(&self.history[i], &self.history[j]);
                corr[i][j] = r;
                corr[j][i] = r;
            }
            corr[i][i] = 1.0;
        }
        corr
    }

    /// Update baseline with exponential moving average
    fn update_baseline(&mut self, alpha: f64) {
        let current = self.current_correlations();
        let n = self.num_channels;
        if self.baseline.len() != n {
            self.baseline = current.clone();
            return;
        }
        for i in 0..n {
            for j in 0..n {
                self.baseline[i][j] = alpha * current[i][j] + (1.0 - alpha) * self.baseline[i][j];
            }
        }
    }

    /// Compute anomaly: how much current correlations deviate from baseline
    fn anomaly_score(&self) -> (f64, Vec<usize>) {
        let current = self.current_correlations();
        let n = self.num_channels;
        if n < 2 { return (0.0, vec![]); }

        let mut total_deviation = 0.0;
        let mut channel_deviation = vec![0.0; n];
        let mut count = 0;

        for i in 0..n {
            for j in (i + 1)..n {
                let dev = (current[i][j] - self.baseline[i][j]).abs();
                total_deviation += dev;
                channel_deviation[i] += dev;
                channel_deviation[j] += dev;
                count += 1;
            }
        }

        let avg_dev = if count > 0 { total_deviation / count as f64 } else { 0.0 };

        // Find affected channels (deviation > 2x average)
        let threshold = avg_dev * 2.0;
        let pairs_per_ch = if n > 1 { (n - 1) as f64 } else { 1.0 };
        let affected: Vec<usize> = channel_deviation.iter().enumerate()
            .filter(|(_, d)| **d / pairs_per_ch > threshold && threshold > 0.01)
            .map(|(i, _)| i)
            .collect();

        (avg_dev, affected)
    }
}

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 { return 0.0; }
    let ma = a.iter().sum::<f64>() / n as f64;
    let mb = b.iter().sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for i in 0..n {
        let da = a[i] - ma;
        let db = b[i] - mb;
        cov += da * db;
        va += da * da;
        vb += db * db;
    }
    let denom = (va * vb).sqrt();
    if denom < 1e-10 { 0.0 } else { cov / denom }
}

#[derive(serde::Serialize)]
struct GhostReport {
    anomaly_score: f64,
    anomaly_level: String,
    affected_channels: Vec<usize>,
    num_channels: usize,
    history_depth: usize,
    timestamp: u64,
}

fn classify_anomaly(score: f64) -> &'static str {
    if score < 0.1 { "quiet" }
    else if score < 0.3 { "mild" }
    else if score < 0.5 { "moderate" }
    else if score < 0.7 { "strong" }
    else { "extreme" }
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

fn run_once(tracker: &mut CorrelationTracker) -> Result<GhostReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples in sensor data")?;

    let values: Vec<f64> = samples.iter()
        .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
        .collect();

    if values.len() < 2 {
        return Err("need at least 2 channels for correlation".into());
    }

    tracker.update(&values);
    let (score, affected) = tracker.anomaly_score();
    tracker.update_baseline(0.05); // Slow baseline adaptation

    let depth = if tracker.history.is_empty() { 0 } else { tracker.history[0].len() };

    let report = GhostReport {
        anomaly_score: score,
        anomaly_level: classify_anomaly(score).into(),
        affected_channels: affected.clone(),
        num_channels: values.len(),
        history_depth: depth,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        score,
        affected.len() as f64 / values.len().max(1) as f64,
        values.len() as f64 / 16.0,
        depth as f64 / BASELINE_WINDOW as f64,
        0.0, 0.0, 0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-ghost-hunter] store error: {e}");
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

    eprintln!("[cog-ghost-hunter] starting (interval={}s)", interval);

    let mut tracker = CorrelationTracker::new();

    loop {
        let start = Instant::now();
        match run_once(&mut tracker) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.anomaly_score > 0.5 {
                    eprintln!("[cog-ghost-hunter] ALERT: {} anomaly (score={:.2}) on channels {:?}",
                        report.anomaly_level, report.anomaly_score, report.affected_channels);
                }
            }
            Err(e) => eprintln!("[cog-ghost-hunter] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
