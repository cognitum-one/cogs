//! Cognitum Cog: Neural Trader
//!
//! Market data analysis. Fetch price data from external API, create 8-dim
//! embeddings (OHLCV + volume + change + spread), store in vector store,
//! query for similar historical patterns. Report signal.
//!
//! Usage:
//!   cog-neural-trader --once
//!   cog-neural-trader --interval 60

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Market candle data
struct Candle {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

impl Candle {
    fn spread(&self) -> f64 {
        self.high - self.low
    }

    fn change(&self) -> f64 {
        if self.open.abs() < 1e-10 { return 0.0; }
        (self.close - self.open) / self.open
    }

    fn body_ratio(&self) -> f64 {
        let spread = self.spread();
        if spread < 1e-10 { return 0.0; }
        (self.close - self.open).abs() / spread
    }

    /// Encode candle as 8-dim normalized vector
    fn to_embedding(&self, price_scale: f64, vol_scale: f64) -> [f64; DIM] {
        let ps = price_scale.max(1e-10);
        let vs = vol_scale.max(1e-10);
        [
            self.open / ps,
            self.high / ps,
            self.low / ps,
            self.close / ps,
            (self.volume / vs).min(1.0),
            self.change(),
            self.spread() / ps,
            self.body_ratio(),
        ]
    }
}

/// Synthesize candle data from sensor readings (sensor data as price proxy)
fn sensors_to_candles(values: &[f64]) -> Vec<Candle> {
    // Group values into candles of 4 samples each
    let chunk_size = 4;
    values
        .chunks(chunk_size)
        .filter(|c| !c.is_empty())
        .map(|chunk| {
            let open = chunk[0];
            let close = *chunk.last().unwrap_or(&open);
            let high = chunk.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let low = chunk.iter().cloned().fold(f64::INFINITY, f64::min);
            let volume = chunk.iter().map(|v| v.abs()).sum::<f64>();
            Candle { open, high, low, close, volume }
        })
        .collect()
}

/// Generate trading signal based on pattern similarity
fn trading_signal(similarity: f64, current_change: f64) -> (&'static str, f64) {
    if similarity < 0.3 {
        ("neutral", 0.0)
    } else if current_change > 0.02 && similarity > 0.7 {
        ("strong_buy", similarity)
    } else if current_change > 0.0 && similarity > 0.5 {
        ("buy", similarity * 0.7)
    } else if current_change < -0.02 && similarity > 0.7 {
        ("strong_sell", -similarity)
    } else if current_change < 0.0 && similarity > 0.5 {
        ("sell", -similarity * 0.7)
    } else {
        ("hold", 0.0)
    }
}

#[derive(serde::Serialize)]
struct TraderResult {
    candle_count: usize,
    latest_embedding: [f64; DIM],
    similar_patterns_found: usize,
    top_similarity: f64,
    signal: String,
    signal_strength: f64,
    latest_change: f64,
    latest_spread: f64,
    anomalies: Vec<String>,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn query_store(vector: &[f64; DIM]) -> Result<Vec<(Vec<f64>, f64)>, String> {
    let payload = serde_json::json!({ "vector": vector, "k": 5, "metric": "cosine" });
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
        let vec: Vec<f64> = r.get("vector").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())?;
        let score = r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
        if vec.len() == DIM { Some((vec, score)) } else { None }
    }).collect())
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

fn run_once() -> Result<TraderResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    let candles = sensors_to_candles(&values);
    if candles.is_empty() { return Err("insufficient data for candles".into()); }

    // Compute scaling factors
    let all_prices: Vec<f64> = candles.iter().flat_map(|c| vec![c.open, c.high, c.low, c.close]).collect();
    let price_scale = all_prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(1.0);
    let vol_scale = candles.iter().map(|c| c.volume).fold(f64::NEG_INFINITY, f64::max).max(1.0);

    let latest = candles.last().unwrap();
    let embedding = latest.to_embedding(price_scale, vol_scale);

    // Query for similar patterns
    let similar = query_store(&embedding).unwrap_or_default();
    let top_sim = similar.first().map(|(_, s)| *s).unwrap_or(0.0);

    let (signal, strength) = trading_signal(top_sim, latest.change());

    let mut anomalies = Vec::new();
    if latest.spread() / price_scale > 0.1 {
        anomalies.push(format!("HIGH_VOLATILITY: spread={:.4}", latest.spread()));
    }
    if latest.change().abs() > 0.05 {
        anomalies.push(format!("LARGE_MOVE: change={:.2}%", latest.change() * 100.0));
    }

    let _ = store_vector(&embedding);

    Ok(TraderResult {
        candle_count: candles.len(),
        latest_embedding: embedding,
        similar_patterns_found: similar.len(),
        top_similarity: top_sim,
        signal: signal.into(),
        signal_strength: strength,
        latest_change: latest.change(),
        latest_spread: latest.spread(),
        anomalies,
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
        .unwrap_or(60);

    eprintln!("[cog-neural-trader] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.anomalies.is_empty() {
                    eprintln!("[cog-neural-trader] ALERT: {:?}", r.anomalies);
                }
            }
            Err(e) => eprintln!("[cog-neural-trader] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
