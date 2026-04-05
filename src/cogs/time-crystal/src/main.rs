//! Cognitum Cog: Time Crystal
//!
//! Detects repeating temporal symmetry patterns using autocorrelation at
//! multiple lags. Finds the strongest periodic component and reports
//! period, strength, and phase.
//!
//! Usage:
//!   cog-time-crystal --once
//!   cog-time-crystal --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const MAX_HISTORY: usize = 256;

struct SignalBuffer {
    values: Vec<f64>,
}

impl SignalBuffer {
    fn new() -> Self { Self { values: Vec::new() } }

    fn push_all(&mut self, vals: &[f64]) {
        let mean = if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 };
        self.values.push(mean);
        if self.values.len() > MAX_HISTORY {
            self.values.remove(0);
        }
    }

    fn len(&self) -> usize { self.values.len() }

    /// Normalized autocorrelation at given lag
    fn autocorrelation(&self, lag: usize) -> f64 {
        let s = &self.values;
        if lag >= s.len() || s.len() < 3 { return 0.0; }
        let n = s.len();
        let mean = s.iter().sum::<f64>() / n as f64;
        let var: f64 = s.iter().map(|v| (v - mean).powi(2)).sum();
        if var < 1e-10 { return 0.0; }
        let cov: f64 = (0..n - lag).map(|i| (s[i] - mean) * (s[i + lag] - mean)).sum();
        cov / var
    }

    /// Find all peaks in autocorrelation (local maxima above threshold)
    fn find_periodic_components(&self, min_lag: usize, max_lag: usize) -> Vec<(usize, f64)> {
        let max_l = max_lag.min(self.values.len() / 2);
        if min_lag >= max_l { return vec![]; }

        let corrs: Vec<f64> = (min_lag..=max_l).map(|lag| self.autocorrelation(lag)).collect();

        let mut peaks = Vec::new();
        for i in 1..corrs.len().saturating_sub(1) {
            if corrs[i] > corrs[i - 1] && corrs[i] > corrs[i + 1] && corrs[i] > 0.1 {
                peaks.push((i + min_lag, corrs[i]));
            }
        }
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        peaks
    }

    /// Estimate phase of the dominant period (position within the cycle)
    fn estimate_phase(&self, period: usize) -> f64 {
        if period == 0 || self.values.is_empty() { return 0.0; }
        let pos = self.values.len() % period;
        pos as f64 / period as f64 * 2.0 * std::f64::consts::PI
    }
}

#[derive(serde::Serialize)]
struct CrystalReport {
    dominant_period: usize,
    dominant_strength: f64,
    phase_radians: f64,
    phase_fraction: f64,
    harmonics: Vec<HarmonicComponent>,
    symmetry_score: f64,
    history_depth: usize,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct HarmonicComponent {
    period: usize,
    strength: f64,
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

fn run_once(buf: &mut SignalBuffer) -> Result<CrystalReport, String> {
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

    buf.push_all(&values);

    let peaks = buf.find_periodic_components(2, MAX_HISTORY / 2);

    let (dom_period, dom_strength) = peaks.first().copied().unwrap_or((0, 0.0));
    let phase = buf.estimate_phase(dom_period);

    let harmonics: Vec<HarmonicComponent> = peaks.iter().take(5)
        .map(|&(p, s)| HarmonicComponent { period: p, strength: s })
        .collect();

    // Symmetry score: ratio of periodic energy to total
    let total_energy: f64 = peaks.iter().map(|(_, s)| s).sum();
    let symmetry = if peaks.is_empty() { 0.0 } else { total_energy / peaks.len() as f64 };

    let report = CrystalReport {
        dominant_period: dom_period,
        dominant_strength: dom_strength,
        phase_radians: phase,
        phase_fraction: phase / (2.0 * std::f64::consts::PI),
        harmonics,
        symmetry_score: symmetry,
        history_depth: buf.len(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    };

    let vec8 = [
        dom_period as f64 / 128.0,
        dom_strength,
        phase / (2.0 * std::f64::consts::PI),
        symmetry,
        buf.len() as f64 / MAX_HISTORY as f64,
        peaks.len() as f64 / 10.0,
        0.0, 0.0,
    ];
    if let Err(e) = store_vector(vec8) {
        eprintln!("[cog-time-crystal] store error: {e}");
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

    eprintln!("[cog-time-crystal] starting (interval={}s)", interval);

    let mut buf = SignalBuffer::new();

    loop {
        let start = Instant::now();
        match run_once(&mut buf) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.dominant_strength > 0.5 {
                    eprintln!("[cog-time-crystal] ALERT: strong periodicity period={} strength={:.2}",
                        report.dominant_period, report.dominant_strength);
                }
            }
            Err(e) => eprintln!("[cog-time-crystal] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
