//! Cognitum Cog: Temporal Compression
//!
//! Compresses old signal data using downsampling + statistical summary.
//! Recent data at full resolution, older data as mean+std per epoch.
//! Reduces storage while preserving signal characteristics.
//!
//! Usage:
//!   cog-temporal-compress --once
//!   cog-temporal-compress --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const FULL_RES_WINDOW: usize = 64;   // Keep last 64 samples at full resolution
const EPOCH_SIZE: usize = 16;         // Compress groups of 16 into summaries
const MAX_EPOCHS: usize = 100;        // Keep max 100 compressed epochs

#[derive(serde::Serialize, Clone)]
struct EpochSummary {
    mean: f64,
    std_dev: f64,
    min: f64,
    max: f64,
    count: usize,
    timestamp: u64,
}

struct TemporalStore {
    /// Full-resolution recent data
    recent: Vec<f64>,
    recent_ts: Vec<u64>,
    /// Compressed older epochs
    epochs: Vec<EpochSummary>,
}

impl TemporalStore {
    fn new() -> Self {
        Self { recent: Vec::new(), recent_ts: Vec::new(), epochs: Vec::new() }
    }

    fn push(&mut self, value: f64, ts: u64) {
        self.recent.push(value);
        self.recent_ts.push(ts);

        // When recent buffer overflows, compress oldest EPOCH_SIZE into a summary
        while self.recent.len() > FULL_RES_WINDOW + EPOCH_SIZE {
            let chunk: Vec<f64> = self.recent.drain(..EPOCH_SIZE).collect();
            let ts_chunk: Vec<u64> = self.recent_ts.drain(..EPOCH_SIZE).collect();

            let mean = chunk.iter().sum::<f64>() / chunk.len() as f64;
            let std_dev = (chunk.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / chunk.len() as f64).sqrt();
            let min = chunk.iter().cloned().fold(f64::MAX, f64::min);
            let max = chunk.iter().cloned().fold(f64::MIN, f64::max);

            self.epochs.push(EpochSummary {
                mean, std_dev, min, max,
                count: chunk.len(),
                timestamp: ts_chunk.first().copied().unwrap_or(0),
            });

            // Cap epochs
            if self.epochs.len() > MAX_EPOCHS {
                // Merge two oldest epochs
                let a = self.epochs.remove(0);
                let b = self.epochs.remove(0);
                let total = a.count + b.count;
                let merged_mean = (a.mean * a.count as f64 + b.mean * b.count as f64) / total as f64;
                // Pooled std dev approximation
                let merged_var = ((a.count as f64 - 1.0) * a.std_dev.powi(2)
                    + (b.count as f64 - 1.0) * b.std_dev.powi(2)
                    + a.count as f64 * (a.mean - merged_mean).powi(2)
                    + b.count as f64 * (b.mean - merged_mean).powi(2))
                    / (total as f64 - 1.0);

                self.epochs.insert(0, EpochSummary {
                    mean: merged_mean,
                    std_dev: merged_var.max(0.0).sqrt(),
                    min: a.min.min(b.min),
                    max: a.max.max(b.max),
                    count: total,
                    timestamp: a.timestamp,
                });
            }
        }
    }

    fn total_samples_represented(&self) -> usize {
        let epoch_count: usize = self.epochs.iter().take(256).map(|e| e.count).sum();
        epoch_count + self.recent.len()
    }

    fn compression_ratio(&self) -> f64 {
        let total = self.total_samples_represented() as f64;
        let stored = (self.recent.len() + self.epochs.len()) as f64;
        if stored < 1.0 { return 1.0; }
        total / stored
    }
}

#[derive(serde::Serialize)]
struct CompressReport {
    recent_count: usize,
    epoch_count: usize,
    total_samples: usize,
    compression_ratio: f64,
    recent_mean: f64,
    recent_std: f64,
    oldest_epoch_ts: u64,
    newest_recent_ts: u64,
    memory_saved_pct: f64,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
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

fn run_once(store: &mut TemporalStore) -> Result<CompressReport, String> {
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

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    // Ingest mean of current frame
    let mean_val = values.iter().sum::<f64>() / values.len() as f64;
    store.push(mean_val, now);

    let recent_mean = if store.recent.is_empty() { 0.0 } else {
        store.recent.iter().sum::<f64>() / store.recent.len() as f64
    };
    let recent_std = if store.recent.len() < 2 { 0.0 } else {
        (store.recent.iter().map(|v| (v - recent_mean).powi(2)).sum::<f64>() / store.recent.len() as f64).sqrt()
    };

    let ratio = store.compression_ratio();
    let saved = if ratio > 1.0 { (1.0 - 1.0 / ratio) * 100.0 } else { 0.0 };

    let report = CompressReport {
        recent_count: store.recent.len(),
        epoch_count: store.epochs.len(),
        total_samples: store.total_samples_represented(),
        compression_ratio: ratio,
        recent_mean,
        recent_std,
        oldest_epoch_ts: store.epochs.first().map(|e| e.timestamp).unwrap_or(0),
        newest_recent_ts: store.recent_ts.last().copied().unwrap_or(0),
        memory_saved_pct: saved,
        timestamp: now,
    };

    let vec8 = [
        ratio / 20.0,
        saved / 100.0,
        store.recent.len() as f64 / FULL_RES_WINDOW as f64,
        store.epochs.len() as f64 / MAX_EPOCHS as f64,
        recent_mean / 100.0,
        recent_std / 50.0,
        store.total_samples_represented() as f64 / 10000.0,
        0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-temporal-compress] store error: {e}");
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

    eprintln!("[cog-temporal-compress] starting (interval={}s, full_res={}, epoch_size={})",
        interval, FULL_RES_WINDOW, EPOCH_SIZE);

    let mut store = TemporalStore::new();

    loop {
        let start = Instant::now();
        match run_once(&mut store) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            }
            Err(e) => eprintln!("[cog-temporal-compress] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
