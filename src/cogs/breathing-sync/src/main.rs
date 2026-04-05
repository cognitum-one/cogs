//! Cognitum Cog: Breathing Sync
//!
//! Compares breathing signals from 2+ sensor channels. Computes
//! cross-correlation and phase coherence. Reports sync ratio (0-1)
//! and phase lag in samples. Requires at least 2 channels from the
//! sensor stream.
//!
//! Usage:
//!   cog-breathing-sync --once
//!   cog-breathing-sync --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

struct BandpassFilter {
    a1: f64, a2: f64, b0: f64, b2: f64,
    x1: f64, x2: f64, y1: f64, y2: f64,
}

impl BandpassFilter {
    fn new(freq_low: f64, freq_high: f64, sample_rate: f64) -> Self {
        let omega_low = 2.0 * std::f64::consts::PI * freq_low / sample_rate;
        let omega_high = 2.0 * std::f64::consts::PI * freq_high / sample_rate;
        let center = (omega_low + omega_high) / 2.0;
        let bandwidth = omega_high - omega_low;
        let r = 1.0 - bandwidth / 2.0;
        let r2 = r * r;
        Self {
            a1: -2.0 * r * center.cos(), a2: r2,
            b0: 1.0 - r2, b2: -(1.0 - r2),
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        }
    }
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = input;
        self.y2 = self.y1; self.y1 = output;
        output
    }
}

/// Normalized cross-correlation between two signals.
/// Returns (max_correlation, lag_at_max) where lag is in samples.
/// Searches lags from -max_lag to +max_lag.
fn cross_correlation(a: &[f64], b: &[f64], max_lag: usize) -> (f64, i64) {
    let n = a.len().min(b.len());
    if n < 2 { return (0.0, 0); }

    let mean_a = a[..n].iter().sum::<f64>() / n as f64;
    let mean_b = b[..n].iter().sum::<f64>() / n as f64;
    let energy_a: f64 = a[..n].iter().map(|v| (v - mean_a).powi(2)).sum();
    let energy_b: f64 = b[..n].iter().map(|v| (v - mean_b).powi(2)).sum();
    let norm = (energy_a * energy_b).sqrt();
    if norm < 1e-15 { return (0.0, 0); }

    let mut best_corr = -2.0_f64;
    let mut best_lag: i64 = 0;

    let search_range = max_lag.min(n / 2);
    for lag_i in 0..=(2 * search_range) {
        let lag = lag_i as i64 - search_range as i64;
        let mut sum = 0.0;
        let mut count = 0;
        for i in 0..n {
            let j = i as i64 + lag;
            if j >= 0 && (j as usize) < n {
                sum += (a[i] - mean_a) * (b[j as usize] - mean_b);
                count += 1;
            }
        }
        if count > 0 {
            let corr = sum / norm;
            if corr > best_corr {
                best_corr = corr;
                best_lag = lag;
            }
        }
    }

    (best_corr.max(-1.0).min(1.0), best_lag)
}

/// Phase coherence: ratio of correlated energy to total energy.
/// Computed as the squared cross-correlation (analogous to coherence^2).
fn phase_coherence(correlation: f64) -> f64 {
    correlation.powi(2).min(1.0)
}

#[derive(serde::Serialize)]
struct SyncReport {
    channels_used: usize,
    sync_ratio: f64,
    phase_lag_samples: i64,
    phase_lag_seconds: f64,
    phase_coherence: f64,
    cross_correlation: f64,
    channel_pair: String,
    alerts: Vec<String>,
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
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_report(report: &SyncReport) -> Result<(), String> {
    let vector = vec![
        report.sync_ratio,
        report.phase_coherence,
        report.cross_correlation.abs(),
        (report.phase_lag_seconds.abs() / 5.0).min(1.0),
        report.channels_used as f64 / 10.0,
        if report.alerts.is_empty() { 0.0 } else { 1.0 },
        0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
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

fn run_once() -> Result<SyncReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    // Group samples by channel
    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0");
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch.to_string()).or_default().push(val);
    }

    // Need at least 2 channels
    let mut ch_names: Vec<String> = channels.keys().cloned().collect();
    ch_names.sort();

    if ch_names.len() < 2 {
        // Fallback: split single channel into odd/even indices
        let all_vals: Vec<f64> = samples.iter()
            .filter_map(|s| s.get("value").and_then(|v| v.as_f64()))
            .collect();
        if all_vals.len() < 4 {
            return Err("need at least 2 channels or 4 samples".into());
        }
        let ch_a: Vec<f64> = all_vals.iter().step_by(2).copied().collect();
        let ch_b: Vec<f64> = all_vals.iter().skip(1).step_by(2).copied().collect();
        channels.clear();
        channels.insert("ch0_even".into(), ch_a);
        channels.insert("ch1_odd".into(), ch_b);
        ch_names = vec!["ch0_even".into(), "ch1_odd".into()];
    }

    let sample_rate = 10.0;

    // Bandpass filter both channels for breathing (0.1-0.5 Hz)
    let mut filter_a = BandpassFilter::new(0.1, 0.5, sample_rate);
    let mut filter_b = BandpassFilter::new(0.1, 0.5, sample_rate);

    let raw_a = &channels[&ch_names[0]];
    let raw_b = &channels[&ch_names[1]];
    let filtered_a: Vec<f64> = raw_a.iter().map(|&v| filter_a.process(v)).collect();
    let filtered_b: Vec<f64> = raw_b.iter().map(|&v| filter_b.process(v)).collect();

    let max_lag = filtered_a.len().min(filtered_b.len()).min(20);
    let (xcorr, lag) = cross_correlation(&filtered_a, &filtered_b, max_lag);
    let coherence = phase_coherence(xcorr);
    let lag_seconds = lag as f64 / sample_rate;

    // Sync ratio: 1.0 = perfectly in sync, 0.0 = no sync
    let sync_ratio = xcorr.max(0.0);

    let mut alerts = Vec::new();
    if sync_ratio < 0.3 && channels.len() >= 2 {
        alerts.push(format!("LOW_SYNC: ratio={:.2} between {} and {}", sync_ratio, ch_names[0], ch_names[1]));
    }
    if lag_seconds.abs() > 2.0 {
        alerts.push(format!("LARGE_PHASE_LAG: {:.2}s between channels", lag_seconds));
    }

    Ok(SyncReport {
        channels_used: ch_names.len(),
        sync_ratio,
        phase_lag_samples: lag,
        phase_lag_seconds: lag_seconds,
        phase_coherence: coherence,
        cross_correlation: xcorr,
        channel_pair: format!("{} vs {}", ch_names[0], ch_names[1]),
        alerts,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-breathing-sync] starting (interval={}s)", interval);

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_report(&report) {
                    eprintln!("[cog-breathing-sync] store error: {e}");
                }
                if !report.alerts.is_empty() {
                    eprintln!("[cog-breathing-sync] ALERT: {:?}", report.alerts);
                }
            }
            Err(e) => eprintln!("[cog-breathing-sync] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
