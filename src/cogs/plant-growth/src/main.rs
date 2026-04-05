//! Cognitum Cog: Plant Growth
//!
//! Tracks slow signal changes over hours/days. Computes daily growth rate
//! from amplitude trend. Detects day/night cycles via periodic component.
//! Maintains rolling 24h statistics.
//!
//! Usage:
//!   cog-plant-growth --once
//!   cog-plant-growth --interval 300   # Every 5 minutes

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 288; // 24h at 5-min intervals

struct RollingHistory {
    values: Vec<f64>,
    timestamps: Vec<u64>,
}

impl RollingHistory {
    fn new() -> Self {
        Self { values: Vec::new(), timestamps: Vec::new() }
    }

    fn push(&mut self, value: f64, ts: u64) {
        self.values.push(value);
        self.timestamps.push(ts);
        if self.values.len() > MAX_HISTORY {
            self.values.remove(0);
            self.timestamps.remove(0);
        }
    }

    fn len(&self) -> usize { self.values.len() }

    /// Linear regression slope (growth rate per sample)
    fn trend_slope(&self) -> f64 {
        let n = self.values.len() as f64;
        if n < 2.0 { return 0.0; }
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = self.values.iter().sum::<f64>() / n;
        let mut num = 0.0;
        let mut den = 0.0;
        for (i, v) in self.values.iter().enumerate() {
            let xi = i as f64 - x_mean;
            num += xi * (v - y_mean);
            den += xi * xi;
        }
        if den.abs() < 1e-10 { 0.0 } else { num / den }
    }

    /// Detect periodicity at ~24h using autocorrelation at lag ~288
    fn day_night_strength(&self, samples_per_day: usize) -> f64 {
        if self.values.len() < samples_per_day + 1 { return 0.0; }
        let mean = self.values.iter().sum::<f64>() / self.values.len() as f64;
        let var: f64 = self.values.iter().map(|v| (v - mean).powi(2)).sum();
        if var < 1e-10 { return 0.0; }
        let n = self.values.len() - samples_per_day;
        let cov: f64 = (0..n).map(|i| {
            (self.values[i] - mean) * (self.values[i + samples_per_day] - mean)
        }).sum();
        (cov / var).max(0.0)
    }

    /// Rolling stats
    fn stats(&self) -> (f64, f64, f64, f64) {
        if self.values.is_empty() { return (0.0, 0.0, 0.0, 0.0); }
        let mean = self.values.iter().sum::<f64>() / self.values.len() as f64;
        let var = self.values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / self.values.len() as f64;
        let min = self.values.iter().cloned().fold(f64::MAX, f64::min);
        let max = self.values.iter().cloned().fold(f64::MIN, f64::max);
        (mean, var.sqrt(), min, max)
    }
}

#[derive(serde::Serialize)]
struct PlantReport {
    growth_rate: f64,
    growth_rate_per_hour: f64,
    day_night_strength: f64,
    is_daytime: bool,
    rolling_mean: f64,
    rolling_std: f64,
    rolling_min: f64,
    rolling_max: f64,
    history_samples: usize,
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

fn run_once(history: &mut RollingHistory, interval_secs: u64) -> Result<PlantReport, String> {
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

    let mean_amplitude = values.iter().sum::<f64>() / values.len() as f64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    history.push(mean_amplitude, now);

    let slope = history.trend_slope();
    let samples_per_hour = 3600.0 / interval_secs as f64;
    let samples_per_day = (samples_per_hour * 24.0) as usize;
    let growth_per_hour = slope * samples_per_hour;

    let dn_strength = history.day_night_strength(samples_per_day);
    let (r_mean, r_std, r_min, r_max) = history.stats();

    // Daytime heuristic: current value above rolling mean
    let is_daytime = mean_amplitude > r_mean;

    let report = PlantReport {
        growth_rate: slope,
        growth_rate_per_hour: growth_per_hour,
        day_night_strength: dn_strength,
        is_daytime,
        rolling_mean: r_mean,
        rolling_std: r_std,
        rolling_min: r_min,
        rolling_max: r_max,
        history_samples: history.len(),
        timestamp: now,
    };

    let vec8 = [
        slope * 1000.0,
        growth_per_hour / 10.0,
        dn_strength,
        if is_daytime { 1.0 } else { 0.0 },
        r_mean / 100.0,
        r_std / 50.0,
        history.len() as f64 / MAX_HISTORY as f64,
        0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-plant-growth] store error: {e}");
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
        .unwrap_or(300);

    eprintln!("[cog-plant-growth] starting (interval={}s, max_history={})", interval, MAX_HISTORY);

    let mut history = RollingHistory::new();

    loop {
        let start = Instant::now();
        match run_once(&mut history, interval) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.growth_rate_per_hour.abs() > 1.0 {
                    eprintln!("[cog-plant-growth] ALERT: rapid growth rate {:.3}/hr", report.growth_rate_per_hour);
                }
            }
            Err(e) => eprintln!("[cog-plant-growth] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
