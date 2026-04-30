//! Cognitum Cog: Vital Trend Monitor
//!
//! Extracts breathing rate and heart rate from the seed's sensor stream,
//! detects anomalies using Welford online statistics, and stores trends
//! in the vector store.
//!
//! Based on wifi-densepose-vitals algorithms (bandpass + zero-crossing).
//!
//! Usage:
//!   cog-vital-trend --once          # Single measurement
//!   cog-vital-trend                 # Continuous monitoring (10s interval)
//!   cog-vital-trend --interval 30   # Custom interval

use std::io::Read;
use std::time::{Duration, Instant};

/// Welford online statistics — numerically stable streaming mean/variance
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

    fn variance(&self) -> f64 {
        if self.count < 2 { return 0.0; }
        self.m2 / (self.count - 1) as f64
    }

    fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { return 0.0; }
        (value - self.mean) / sd
    }
}

/// 2nd-order IIR bandpass filter for vital sign extraction
struct BandpassFilter {
    // Filter coefficients
    a1: f64,
    a2: f64,
    b0: f64,
    b2: f64,
    // State
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BandpassFilter {
    /// Create bandpass filter for given frequency range at sample_rate
    fn new(freq_low: f64, freq_high: f64, sample_rate: f64) -> Self {
        let omega_low = 2.0 * std::f64::consts::PI * freq_low / sample_rate;
        let omega_high = 2.0 * std::f64::consts::PI * freq_high / sample_rate;
        let center = (omega_low + omega_high) / 2.0;
        let bandwidth = omega_high - omega_low;
        let r = 1.0 - bandwidth / 2.0;
        let r2 = r * r;

        Self {
            a1: -2.0 * r * center.cos(),
            a2: r2,
            b0: 1.0 - r2,
            b2: -(1.0 - r2),
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        }
    }

    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// Count zero crossings in a signal to estimate frequency
fn zero_crossing_bpm(signal: &[f64], sample_rate: f64) -> f64 {
    if signal.len() < 4 { return 0.0; }
    let mut crossings = 0;
    for i in 1..signal.len() {
        if (signal[i - 1] >= 0.0 && signal[i] < 0.0) ||
           (signal[i - 1] < 0.0 && signal[i] >= 0.0) {
            crossings += 1;
        }
    }
    let duration_s = signal.len() as f64 / sample_rate;
    if duration_s < 0.1 { return 0.0; }
    let freq_hz = crossings as f64 / (2.0 * duration_s);
    freq_hz * 60.0 // Convert to BPM
}

/// Vital sign measurement result
#[derive(serde::Serialize)]
struct VitalMeasurement {
    breathing_bpm: f64,
    breathing_confidence: f64,
    heart_rate_bpm: f64,
    heart_rate_confidence: f64,
    breathing_status: String,
    heart_rate_status: String,
    anomalies: Vec<String>,
    timestamp: u64,
}

/// Fetch sensor stream from cognitum-agent
fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

/// Store vital measurement as vector in the seed store
fn store_measurement(measurement: &VitalMeasurement) -> Result<(), String> {
    let vector = vec![
        measurement.breathing_bpm / 30.0,    // Normalize to ~[0,1]
        measurement.heart_rate_bpm / 120.0,
        measurement.breathing_confidence,
        measurement.heart_rate_confidence,
        if measurement.anomalies.is_empty() { 0.0 } else { 1.0 },
        0.0, 0.0, 0.0, // Padding to dim=8
    ];

    let payload = serde_json::json!({
        "vectors": [[0, vector]],
        "dedup": true
    });

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

fn run_once() -> Result<VitalMeasurement, String> {
    // Fetch sensor data
    let sensors = fetch_sensors()?;

    // Extract channel readings
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples in sensor data")?;

    // Collect amplitude values from all samples as a time series proxy
    let mut amplitudes: Vec<f64> = Vec::new();
    for ch in samples.iter().take(256) {
        if let Some(val) = ch.get("value").and_then(|v| v.as_f64()) {
            amplitudes.push(val);
        }
    }

    if amplitudes.is_empty() {
        return Err("no sensor readings".into());
    }

    // Simulate CSI-like processing on available sensor data
    let sample_rate = 10.0; // 10 Hz synthetic sensors

    // Breathing extraction (0.1-0.5 Hz bandpass)
    let mut breathing_filter = BandpassFilter::new(0.1, 0.5, sample_rate);
    let breathing_signal: Vec<f64> = amplitudes.iter()
        .map(|&v| breathing_filter.process(v))
        .collect();
    let breathing_bpm = zero_crossing_bpm(&breathing_signal, sample_rate);

    // Heart rate extraction (0.8-2.0 Hz bandpass)
    let mut hr_filter = BandpassFilter::new(0.8, 2.0, sample_rate);
    let hr_signal: Vec<f64> = amplitudes.iter()
        .map(|&v| hr_filter.process(v))
        .collect();
    let heart_rate_bpm = zero_crossing_bpm(&hr_signal, sample_rate);

    // Confidence based on signal variance
    let breathing_var: f64 = breathing_signal.iter().map(|v| v * v).sum::<f64>() / breathing_signal.len().max(1) as f64;
    let hr_var: f64 = hr_signal.iter().map(|v| v * v).sum::<f64>() / hr_signal.len().max(1) as f64;
    let breathing_conf = (breathing_var * 100.0).min(1.0);
    let hr_conf = (hr_var * 100.0).min(1.0);

    // Anomaly detection
    let mut anomalies = Vec::new();
    let mut stats = WelfordStats::new();
    for &v in &amplitudes {
        stats.update(v);
    }

    if breathing_bpm > 0.0 && breathing_bpm < 4.0 {
        anomalies.push("APNEA: breathing rate critically low".into());
    }
    if breathing_bpm > 30.0 {
        anomalies.push("TACHYPNEA: breathing rate dangerously high".into());
    }
    if heart_rate_bpm > 100.0 {
        anomalies.push("TACHYCARDIA: heart rate elevated".into());
    }
    if heart_rate_bpm > 0.0 && heart_rate_bpm < 50.0 {
        anomalies.push("BRADYCARDIA: heart rate low".into());
    }

    // Check for sudden changes
    if let Some(last) = amplitudes.last() {
        let z = stats.z_score(*last);
        if z.abs() > 2.5 {
            anomalies.push(format!("SUDDEN_CHANGE: z-score={:.2}", z));
        }
    }

    let breathing_status = if breathing_conf >= 0.7 { "Valid" }
        else if breathing_conf >= 0.4 { "Degraded" }
        else { "Unreliable" };

    let hr_status = if hr_conf >= 0.7 { "Valid" }
        else if hr_conf >= 0.4 { "Degraded" }
        else { "Unreliable" };

    let measurement = VitalMeasurement {
        breathing_bpm,
        breathing_confidence: breathing_conf,
        heart_rate_bpm,
        heart_rate_confidence: hr_conf,
        breathing_status: breathing_status.into(),
        heart_rate_status: hr_status.into(),
        anomalies,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    Ok(measurement)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-vital-trend] starting (interval={}s, once={})", interval, once);

    loop {
        let start = Instant::now();

        match run_once() {
            Ok(m) => {
                println!("{}", serde_json::to_string(&m).unwrap_or_default());

                // Store in vector store
                if let Err(e) = store_measurement(&m) {
                    eprintln!("[cog-vital-trend] store error: {e}");
                }

                if !m.anomalies.is_empty() {
                    eprintln!("[cog-vital-trend] ALERT: {:?}", m.anomalies);
                }
            }
            Err(e) => {
                eprintln!("[cog-vital-trend] error: {e}");
            }
        }

        if once { break; }

        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
