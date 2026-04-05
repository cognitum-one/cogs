//! Cognitum Cog: Coherence Gate
//!
//! Filters signals by coherence. Computes channel-pair correlation.
//! Gate: only passes signals where coherence > threshold (clean data).
//! Reports SNR estimate.
//!
//! Usage:
//!   cog-coherence-gate --once
//!   cog-coherence-gate --interval 10
//!   cog-coherence-gate --threshold 0.7

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 50;

struct CoherenceTracker {
    history: Vec<Vec<f64>>,
    num_channels: usize,
}

impl CoherenceTracker {
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

    /// Mean pairwise coherence per channel
    fn channel_coherences(&self) -> Vec<f64> {
        let n = self.num_channels;
        if n < 2 { return vec![1.0; n]; }

        let mut ch_coh = vec![0.0; n];
        let mut ch_count = vec![0u32; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let coh = Self::pearson(&self.history[i], &self.history[j]).abs();
                ch_coh[i] += coh;
                ch_coh[j] += coh;
                ch_count[i] += 1;
                ch_count[j] += 1;
            }
        }

        ch_coh.iter().zip(ch_count.iter())
            .map(|(&c, &cnt)| if cnt > 0 { c / cnt as f64 } else { 0.0 })
            .collect()
    }

    /// Estimate SNR from coherence: high coherence = signal, low = noise
    fn estimate_snr(&self) -> f64 {
        let coherences = self.channel_coherences();
        if coherences.is_empty() { return 0.0; }
        let mean_coh = coherences.iter().sum::<f64>() / coherences.len() as f64;
        // SNR in dB: map coherence 0-1 to ~0-30 dB
        if mean_coh < 1e-6 { return 0.0; }
        10.0 * (mean_coh / (1.0 - mean_coh + 1e-10)).log10()
    }

    fn depth(&self) -> usize {
        self.history.first().map(|h| h.len()).unwrap_or(0)
    }
}

#[derive(serde::Serialize)]
struct CoherenceReport {
    gated_channels: Vec<usize>,
    rejected_channels: Vec<usize>,
    channel_coherences: Vec<ChannelCoherence>,
    mean_coherence: f64,
    snr_db: f64,
    gate_threshold: f64,
    pass_rate: f64,
    history_depth: usize,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct ChannelCoherence {
    channel: usize,
    coherence: f64,
    passed: bool,
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

fn run_once(tracker: &mut CoherenceTracker, threshold: f64) -> Result<CoherenceReport, String> {
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

    tracker.update(&values);
    let coherences = tracker.channel_coherences();
    let snr = tracker.estimate_snr();

    let mut gated = Vec::new();
    let mut rejected = Vec::new();
    let mut channel_cohs = Vec::new();

    for (i, &coh) in coherences.iter().enumerate() {
        let passed = coh >= threshold;
        if passed { gated.push(i); } else { rejected.push(i); }
        channel_cohs.push(ChannelCoherence { channel: i, coherence: coh, passed });
    }

    let mean_coh = if coherences.is_empty() { 0.0 } else {
        coherences.iter().sum::<f64>() / coherences.len() as f64
    };
    let pass_rate = gated.len() as f64 / values.len().max(1) as f64;

    let report = CoherenceReport {
        gated_channels: gated,
        rejected_channels: rejected,
        channel_coherences: channel_cohs,
        mean_coherence: mean_coh,
        snr_db: snr,
        gate_threshold: threshold,
        pass_rate,
        history_depth: tracker.depth(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        mean_coh,
        snr / 30.0,
        pass_rate,
        threshold,
        values.len() as f64 / 16.0,
        tracker.depth() as f64 / MAX_HISTORY as f64,
        0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-coherence-gate] store error: {e}");
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
    let threshold = args.iter()
        .position(|a| a == "--threshold")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.7);

    eprintln!("[cog-coherence-gate] starting (interval={}s, threshold={:.2})", interval, threshold);

    let mut tracker = CoherenceTracker::new();

    loop {
        let start = Instant::now();
        match run_once(&mut tracker, threshold) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.pass_rate < 0.5 {
                    eprintln!("[cog-coherence-gate] ALERT: low coherence, {:.0}% channels gated out",
                        (1.0 - report.pass_rate) * 100.0);
                }
            }
            Err(e) => eprintln!("[cog-coherence-gate] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
