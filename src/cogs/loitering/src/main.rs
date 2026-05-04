//! Cognitum Cog: Loitering Detection
//!
//! State machine that detects when someone remains stationary in a zone
//! beyond a configurable time threshold. Uses presence signal variance
//! combined with motion estimation from CSI amplitude change rate.
//!
//! Usage:
//!   cog-loitering --once
//!   cog-loitering --interval 10 --loiter-time 120

use std::io::Read;
use std::time::{Duration, Instant};

#[derive(PartialEq, Clone, Copy)]
enum State {
    Empty,
    Moving,
    Stationary,
    Loitering,
}

struct LoiterDetector {
    state: State,
    stationary_start: Option<Instant>,
    loiter_threshold_secs: u64,
    motion_threshold: f64,
    prev_amplitudes: Vec<f64>,
    alert_count: u64,
}

impl LoiterDetector {
    fn new(loiter_threshold_secs: u64) -> Self {
        Self {
            state: State::Empty,
            stationary_start: None,
            loiter_threshold_secs,
            motion_threshold: 5.0,
            prev_amplitudes: Vec::new(),
            alert_count: 0,
        }
    }

    fn update(&mut self, present: bool, amplitudes: &[f64]) -> (State, Option<u64>) {
        // Estimate motion from amplitude change rate
        let motion = if !self.prev_amplitudes.is_empty() && self.prev_amplitudes.len() == amplitudes.len() {
            let sum: f64 = amplitudes.iter().zip(&self.prev_amplitudes)
                .map(|(a, b)| (a - b).abs())
                .sum();
            sum / amplitudes.len() as f64
        } else {
            0.0
        };
        self.prev_amplitudes = amplitudes.to_vec();

        let is_moving = motion > self.motion_threshold;
        let mut loiter_duration = None;

        self.state = match (self.state, present, is_moving) {
            (_, false, _) => {
                self.stationary_start = None;
                State::Empty
            }
            (_, true, true) => {
                self.stationary_start = None;
                State::Moving
            }
            (State::Empty | State::Moving, true, false) => {
                self.stationary_start = Some(Instant::now());
                State::Stationary
            }
            (State::Stationary, true, false) => {
                if let Some(start) = self.stationary_start {
                    let elapsed = start.elapsed().as_secs();
                    if elapsed >= self.loiter_threshold_secs {
                        self.alert_count += 1;
                        loiter_duration = Some(elapsed);
                        State::Loitering
                    } else {
                        State::Stationary
                    }
                } else {
                    State::Stationary
                }
            }
            (State::Loitering, true, false) => {
                if let Some(start) = self.stationary_start {
                    loiter_duration = Some(start.elapsed().as_secs());
                }
                State::Loitering
            }
        };

        (self.state, loiter_duration)
    }
}

#[derive(serde::Serialize)]
struct LoiterReport {
    state: String,
    loitering: bool,
    loiter_duration_secs: Option<u64>,
    alert_count: u64,
    motion_level: f64,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_loiter(report: &LoiterReport) -> Result<(), String> {
    let vector = vec![
        if report.loitering { 1.0 } else { 0.0 },
        report.loiter_duration_secs.unwrap_or(0) as f64 / 600.0,
        report.motion_level / 100.0,
        report.alert_count as f64 / 100.0,
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);
    let loiter_time = args.iter()
        .position(|a| a == "--loiter-time")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120);

    eprintln!("[cog-loitering] starting (interval={}s, loiter_time={}s)", interval, loiter_time);
    let mut detector = LoiterDetector::new(loiter_time);

    // Simple presence check: variance > threshold
    let presence_threshold = 10.0;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples")
                    .and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();

                    // Compute variance for presence
                    let mean = amps.iter().sum::<f64>() / amps.len().max(1) as f64;
                    let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len().max(1) as f64;
                    let present = var > presence_threshold;

                    let (state, duration) = detector.update(present, &amps);
                    let motion = if detector.prev_amplitudes.len() > 1 {
                        let sum: f64 = amps.iter().zip(&detector.prev_amplitudes)
                            .map(|(a, b)| (a - b).abs()).sum();
                        sum / amps.len() as f64
                    } else { 0.0 };

                    let state_str = match state {
                        State::Empty => "empty",
                        State::Moving => "moving",
                        State::Stationary => "stationary",
                        State::Loitering => "LOITERING",
                    };

                    let report = LoiterReport {
                        state: state_str.into(),
                        loitering: state == State::Loitering,
                        loiter_duration_secs: duration,
                        alert_count: detector.alert_count,
                        motion_level: motion,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_loiter(&report) {
                        eprintln!("[cog-loitering] store error: {e}");
                    }
                    if state == State::Loitering {
                        eprintln!("[cog-loitering] ALERT: loitering detected for {}s", duration.unwrap_or(0));
                    }
                }
            }
            Err(e) => eprintln!("[cog-loitering] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
