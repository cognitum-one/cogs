//! Cognitum Cog: Flash Attention
//!
//! Adaptive sensing: identifies which channels carry the most information
//! (highest variance/entropy). Focuses processing on top-K channels.
//! Reports channel importance ranking.
//!
//! Usage:
//!   cog-flash-attention --once
//!   cog-flash-attention --interval 10
//!   cog-flash-attention --top-k 4

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 100;

struct ChannelTracker {
    /// Per-channel value history
    history: Vec<Vec<f64>>,
    num_channels: usize,
}

impl ChannelTracker {
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

    /// Compute variance for each channel
    fn channel_variances(&self) -> Vec<f64> {
        self.history.iter().map(|h| {
            if h.len() < 2 { return 0.0; }
            let mean = h.iter().sum::<f64>() / h.len() as f64;
            h.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (h.len() - 1) as f64
        }).collect()
    }

    /// Compute entropy for each channel (binned histogram)
    fn channel_entropies(&self) -> Vec<f64> {
        self.history.iter().map(|h| {
            if h.len() < 2 { return 0.0; }
            let min = h.iter().cloned().fold(f64::MAX, f64::min);
            let max = h.iter().cloned().fold(f64::MIN, f64::max);
            let range = max - min;
            if range < 1e-10 { return 0.0; }

            // 10 bins
            let num_bins = 10;
            let mut bins = vec![0u32; num_bins];
            for &v in h {
                let idx = ((v - min) / range * (num_bins - 1) as f64) as usize;
                bins[idx.min(num_bins - 1)] += 1;
            }

            let n = h.len() as f64;
            let mut entropy = 0.0;
            for &count in &bins {
                if count > 0 {
                    let p = count as f64 / n;
                    entropy -= p * p.ln();
                }
            }
            entropy
        }).collect()
    }

    /// Compute information score: weighted combination of variance and entropy
    fn channel_importance(&self) -> Vec<(usize, f64)> {
        let variances = self.channel_variances();
        let entropies = self.channel_entropies();

        // Normalize each metric to [0, 1]
        let max_var = variances.iter().cloned().fold(0.0_f64, f64::max).max(1e-10);
        let max_ent = entropies.iter().cloned().fold(0.0_f64, f64::max).max(1e-10);

        let mut scores: Vec<(usize, f64)> = variances.iter().zip(entropies.iter())
            .enumerate()
            .map(|(i, (v, e))| {
                let norm_v = v / max_var;
                let norm_e = e / max_ent;
                (i, norm_v * 0.5 + norm_e * 0.5)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    fn depth(&self) -> usize {
        self.history.first().map(|h| h.len()).unwrap_or(0)
    }
}

#[derive(serde::Serialize)]
struct AttentionReport {
    top_channels: Vec<ChannelRank>,
    num_channels: usize,
    focused_channels: Vec<usize>,
    attention_ratio: f64,
    history_depth: usize,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct ChannelRank {
    channel: usize,
    importance: f64,
    variance: f64,
    entropy: f64,
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

fn run_once(tracker: &mut ChannelTracker, top_k: usize) -> Result<AttentionReport, String> {
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

    let importance = tracker.channel_importance();
    let variances = tracker.channel_variances();
    let entropies = tracker.channel_entropies();

    let k = top_k.min(values.len());
    let focused: Vec<usize> = importance.iter().take(k).map(|&(ch, _)| ch).collect();

    let top_channels: Vec<ChannelRank> = importance.iter().map(|&(ch, imp)| {
        ChannelRank {
            channel: ch,
            importance: imp,
            variance: variances.get(ch).copied().unwrap_or(0.0),
            entropy: entropies.get(ch).copied().unwrap_or(0.0),
        }
    }).collect();

    // Attention ratio: how much info is concentrated in top-K
    let total_imp: f64 = importance.iter().map(|&(_, i)| i).sum();
    let top_imp: f64 = importance.iter().take(k).map(|&(_, i)| i).sum();
    let attention_ratio = if total_imp > 1e-10 { top_imp / total_imp } else { 0.0 };

    let report = AttentionReport {
        top_channels,
        num_channels: values.len(),
        focused_channels: focused,
        attention_ratio,
        history_depth: tracker.depth(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    // Store importance of top channels
    let mut vec8 = [0.0_f64; 8];
    for (i, &(_, imp)) in importance.iter().take(8).enumerate() {
        vec8[i] = imp;
    }
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-flash-attention] store error: {e}");
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
    let top_k = args.iter()
        .position(|a| a == "--top-k")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    eprintln!("[cog-flash-attention] starting (interval={}s, top_k={})", interval, top_k);

    let mut tracker = ChannelTracker::new();

    loop {
        let start = Instant::now();
        match run_once(&mut tracker, top_k) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-flash-attention] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
