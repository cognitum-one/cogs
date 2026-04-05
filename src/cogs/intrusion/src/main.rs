//! Cognitum Cog: Intrusion Detection
//!
//! Alerts when an unauthorized person enters a monitored zone.
//! Uses baseline learning during "armed" period, then detects
//! deviations indicating human entry. Supports scheduled arming
//! and multi-zone monitoring via multiple ESP32 nodes.
//!
//! Usage:
//!   cog-intrusion --once
//!   cog-intrusion --interval 3 --arm-after 60

use std::io::Read;
use std::time::{Duration, Instant};

struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordStats {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }
    fn std_dev(&self) -> f64 {
        if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() }
    }
    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { 0.0 } else { (value - self.mean) / sd }
    }
}

#[derive(PartialEq)]
enum ArmState {
    Learning,
    Armed,
    Alarm,
}

struct IntrusionDetector {
    state: ArmState,
    baseline: WelfordStats,
    variance_baseline: WelfordStats,
    learn_start: Instant,
    arm_after_secs: u64,
    detection_threshold: f64,
    alarm_start: Option<Instant>,
    total_alarms: u64,
    consecutive_triggers: u32,
    trigger_threshold: u32,
}

impl IntrusionDetector {
    fn new(arm_after_secs: u64) -> Self {
        Self {
            state: ArmState::Learning,
            baseline: WelfordStats::new(),
            variance_baseline: WelfordStats::new(),
            learn_start: Instant::now(),
            arm_after_secs,
            detection_threshold: 3.0,
            alarm_start: None,
            total_alarms: 0,
            consecutive_triggers: 0,
            trigger_threshold: 2,
        }
    }

    fn update(&mut self, signal_variance: f64, mean_amplitude: f64) -> IntrusionStatus {
        match self.state {
            ArmState::Learning => {
                self.baseline.update(mean_amplitude);
                self.variance_baseline.update(signal_variance);
                if self.learn_start.elapsed().as_secs() >= self.arm_after_secs {
                    self.state = ArmState::Armed;
                    IntrusionStatus::Armed { baseline_samples: self.baseline.count }
                } else {
                    IntrusionStatus::Learning {
                        progress_pct: (self.learn_start.elapsed().as_secs() as f64 / self.arm_after_secs as f64 * 100.0).min(100.0),
                    }
                }
            }
            ArmState::Armed => {
                let z_amp = self.baseline.z_score(mean_amplitude);
                let z_var = self.variance_baseline.z_score(signal_variance);
                let combined_score = (z_amp.abs() + z_var.abs()) / 2.0;

                if combined_score > self.detection_threshold {
                    self.consecutive_triggers += 1;
                    if self.consecutive_triggers >= self.trigger_threshold {
                        self.state = ArmState::Alarm;
                        self.alarm_start = Some(Instant::now());
                        self.total_alarms += 1;
                        IntrusionStatus::Intrusion {
                            confidence: (combined_score / 5.0).min(1.0),
                            z_amplitude: z_amp,
                            z_variance: z_var,
                        }
                    } else {
                        IntrusionStatus::Suspicious { score: combined_score }
                    }
                } else {
                    self.consecutive_triggers = 0;
                    // Slowly adapt baseline
                    self.baseline.update(mean_amplitude);
                    self.variance_baseline.update(signal_variance);
                    IntrusionStatus::Clear
                }
            }
            ArmState::Alarm => {
                let z_amp = self.baseline.z_score(mean_amplitude);
                let z_var = self.variance_baseline.z_score(signal_variance);
                let combined = (z_amp.abs() + z_var.abs()) / 2.0;

                if combined < self.detection_threshold * 0.5 {
                    self.consecutive_triggers = 0;
                    self.state = ArmState::Armed;
                    IntrusionStatus::Clear
                } else {
                    IntrusionStatus::Intrusion {
                        confidence: (combined / 5.0).min(1.0),
                        z_amplitude: z_amp,
                        z_variance: z_var,
                    }
                }
            }
        }
    }
}

enum IntrusionStatus {
    Learning { progress_pct: f64 },
    Armed { baseline_samples: u64 },
    Clear,
    Suspicious { score: f64 },
    Intrusion { confidence: f64, z_amplitude: f64, z_variance: f64 },
}

#[derive(serde::Serialize)]
struct IntrusionReport {
    status: String,
    armed: bool,
    intrusion_detected: bool,
    confidence: f64,
    z_amplitude: f64,
    z_variance: f64,
    total_alarms: u64,
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

fn store_intrusion(report: &IntrusionReport) -> Result<(), String> {
    let vector = vec![
        if report.intrusion_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.z_amplitude.abs() / 5.0,
        report.z_variance.abs() / 5.0,
        report.total_alarms as f64 / 100.0,
        if report.armed { 1.0 } else { 0.0 },
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(3);
    let arm_after = args.iter()
        .position(|a| a == "--arm-after")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);

    eprintln!("[cog-intrusion] starting (interval={}s, arm_after={}s)", interval, arm_after);
    let mut detector = IntrusionDetector::new(arm_after);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();
                    if amps.is_empty() { continue; }

                    let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                    let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;

                    let status = detector.update(var, mean);
                    let (status_str, armed, intrusion, conf, z_a, z_v) = match status {
                        IntrusionStatus::Learning { progress_pct } => (format!("learning ({:.0}%)", progress_pct), false, false, 0.0, 0.0, 0.0),
                        IntrusionStatus::Armed { .. } => ("armed".into(), true, false, 0.0, 0.0, 0.0),
                        IntrusionStatus::Clear => ("clear".into(), true, false, 0.0, 0.0, 0.0),
                        IntrusionStatus::Suspicious { score } => (format!("suspicious ({:.1})", score), true, false, score / 5.0, 0.0, 0.0),
                        IntrusionStatus::Intrusion { confidence, z_amplitude, z_variance } =>
                            ("INTRUSION".into(), true, true, confidence, z_amplitude, z_variance),
                    };

                    let report = IntrusionReport {
                        status: status_str,
                        armed,
                        intrusion_detected: intrusion,
                        confidence: conf,
                        z_amplitude: z_a,
                        z_variance: z_v,
                        total_alarms: detector.total_alarms,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_intrusion(&report) {
                        eprintln!("[cog-intrusion] store error: {e}");
                    }
                    if intrusion {
                        eprintln!("[cog-intrusion] ALARM: intrusion detected (confidence={:.0}%)", conf * 100.0);
                    }
                }
            }
            Err(e) => eprintln!("[cog-intrusion] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
