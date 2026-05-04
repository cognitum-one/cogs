//! Cognitum Cog: Anomaly Detection
//!
//! Welford online statistics + CUSUM drift detection on sensor streams.
//! Detects sudden changes, gradual drift, and threshold breaches.
//!
//! Usage:
//!   cog-anomaly-detect --once          # Single check
//!   cog-anomaly-detect                 # Continuous (10s)
//!   cog-anomaly-detect --interval 5    # Custom interval
//!   cog-anomaly-detect --threshold 3.0 # Custom z-score threshold

use std::io::Read;
use std::time::{Duration, Instant};

/// Welford online statistics
struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordStats {
    fn new() -> Self {
        Self { count: 0, mean: 0.0, m2: 0.0 }
    }

    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn std_dev(&self) -> f64 {
        if self.count < 2 { return 0.0; }
        (self.m2 / (self.count - 1) as f64).sqrt()
    }

    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { return 0.0; }
        (value - self.mean) / sd
    }
}

/// CUSUM (Cumulative Sum) drift detector
struct Cusum {
    threshold: f64,
    drift: f64,
    s_pos: f64,
    s_neg: f64,
    alarm_pos: bool,
    alarm_neg: bool,
}

impl Cusum {
    fn new(threshold: f64, drift: f64) -> Self {
        Self { threshold, drift, s_pos: 0.0, s_neg: 0.0, alarm_pos: false, alarm_neg: false }
    }

    fn update(&mut self, value: f64, expected: f64) {
        let deviation = value - expected;
        self.s_pos = (self.s_pos + deviation - self.drift).max(0.0);
        self.s_neg = (self.s_neg - deviation - self.drift).max(0.0);
        self.alarm_pos = self.s_pos > self.threshold;
        self.alarm_neg = self.s_neg > self.threshold;
    }

    fn reset(&mut self) {
        self.s_pos = 0.0;
        self.s_neg = 0.0;
        self.alarm_pos = false;
        self.alarm_neg = false;
    }
}

#[derive(serde::Serialize)]
struct AnomalyReport {
    anomalies: Vec<Anomaly>,
    stats: ChannelStats,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct Anomaly {
    #[serde(rename = "type")]
    anomaly_type: String,
    channel: usize,
    value: f64,
    z_score: f64,
    description: String,
}

#[derive(serde::Serialize)]
struct ChannelStats {
    num_channels: usize,
    mean_amplitude: f64,
    max_z_score: f64,
    cusum_drift_detected: bool,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_anomaly(report: &AnomalyReport) -> Result<(), String> {
    let vector = vec![
        report.stats.mean_amplitude / 100.0,
        report.stats.max_z_score / 5.0,
        if report.stats.cusum_drift_detected { 1.0 } else { 0.0 },
        report.anomalies.len() as f64 / 10.0,
        0.0, 0.0, 0.0, 0.0,
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

fn run_once(z_threshold: f64, cusum: &mut Cusum) -> Result<AnomalyReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples in sensor data")?;

    let mut stats = WelfordStats::new();
    let mut anomalies = Vec::new();
    let mut amplitudes: Vec<(usize, f64)> = Vec::new();

    for (i, ch) in samples.iter().take(256).enumerate() {
        if let Some(val) = ch.get("value").and_then(|v| v.as_f64()) {
            amplitudes.push((i, val));
            stats.update(val);
        }
    }

    if amplitudes.is_empty() {
        return Err("no sensor readings".into());
    }

    // Z-score anomalies
    let mut max_z: f64 = 0.0;
    for &(ch_idx, val) in &amplitudes {
        let z = stats.z_score(val);
        if z.abs() > max_z.abs() { max_z = z; }
        if z.abs() > z_threshold {
            anomalies.push(Anomaly {
                anomaly_type: if z > 0.0 { "SPIKE".into() } else { "DROP".into() },
                channel: ch_idx,
                value: val,
                z_score: z,
                description: format!("Channel {} value {:.2} deviates {:.1} sigma from mean", ch_idx, val, z.abs()),
            });
        }
    }

    // CUSUM drift detection on mean
    cusum.update(stats.mean, 0.0);
    let drift_detected = cusum.alarm_pos || cusum.alarm_neg;
    if drift_detected {
        anomalies.push(Anomaly {
            anomaly_type: "DRIFT".into(),
            channel: 0,
            value: stats.mean,
            z_score: 0.0,
            description: format!("CUSUM drift detected: mean={:.2}, s+={:.2}, s-={:.2}",
                stats.mean, cusum.s_pos, cusum.s_neg),
        });
        cusum.reset();
    }

    // Flatline detection
    if stats.std_dev() < 1e-6 && stats.count > 4 {
        anomalies.push(Anomaly {
            anomaly_type: "FLATLINE".into(),
            channel: 0,
            value: stats.mean,
            z_score: 0.0,
            description: "All channels identical — possible sensor failure".into(),
        });
    }

    Ok(AnomalyReport {
        anomalies,
        stats: ChannelStats {
            num_channels: amplitudes.len(),
            mean_amplitude: stats.mean,
            max_z_score: max_z,
            cusum_drift_detected: drift_detected,
        },
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
    let z_threshold = args.iter()
        .position(|a| a == "--threshold")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(2.5);

    eprintln!("[cog-anomaly-detect] starting (interval={}s, z_threshold={:.1})", interval, z_threshold);

    let mut cusum = Cusum::new(5.0, 0.5);

    loop {
        let start = Instant::now();

        match run_once(z_threshold, &mut cusum) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_anomaly(&report) {
                    eprintln!("[cog-anomaly-detect] store error: {e}");
                }
                if !report.anomalies.is_empty() {
                    eprintln!("[cog-anomaly-detect] ALERT: {} anomalies detected", report.anomalies.len());
                }
            }
            Err(e) => eprintln!("[cog-anomaly-detect] error: {e}"),
        }

        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
