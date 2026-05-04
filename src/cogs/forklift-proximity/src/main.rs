//! Cognitum Cog: Forklift Proximity Detection
//!
//! Detects large moving objects (forklifts) via strong multi-channel signal
//! disruption. Alerts when object approaches worker zone (signal strength
//! increases rapidly across multiple channels simultaneously).
//!
//! Usage:
//!   cog-forklift-proximity --once
//!   cog-forklift-proximity --interval 1

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const DISRUPTION_THRESHOLD: f64 = 25.0;   // Multi-channel variance jump
const RATE_THRESHOLD: f64 = 5.0;          // Rate of change threshold
const MIN_CHANNELS_DISRUPTED: usize = 3;  // Minimum channels showing disruption

struct SignalHistory {
    /// Per-channel running mean and variance
    channels: Vec<ChannelState>,
    prev_aggregate_power: f64,
    alert_cooldown: u32,
}

struct ChannelState {
    mean: f64,
    m2: f64,
    count: u64,
    prev_value: f64,
}

impl ChannelState {
    fn new() -> Self { Self { mean: 0.0, m2: 0.0, count: 0, prev_value: 0.0 } }

    fn update(&mut self, val: f64) {
        self.count += 1;
        let delta = val - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = val - self.mean;
        self.m2 += delta * delta2;
        self.prev_value = val;
    }

    fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }

    fn is_disrupted(&self, val: f64) -> bool {
        if self.count < 5 { return false; }
        let std_dev = self.variance().sqrt().max(0.1);
        let z_score = (val - self.mean).abs() / std_dev;
        z_score > 3.0  // >3 sigma departure
    }
}

impl SignalHistory {
    fn new() -> Self {
        Self {
            channels: Vec::new(),
            prev_aggregate_power: 0.0,
            alert_cooldown: 0,
        }
    }
}

#[derive(serde::Serialize)]
struct ProximityReport {
    forklift_detected: bool,
    proximity_level: String,   // "safe", "warning", "danger"
    disrupted_channels: usize,
    total_channels: usize,
    aggregate_power: f64,
    power_rate_of_change: f64,
    signal_disruption: f64,
    alert: bool,
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_detection(report: &ProximityReport) -> Result<(), String> {
    let vector = vec![
        if report.forklift_detected { 1.0 } else { 0.0 },
        match report.proximity_level.as_str() {
            "danger" => 1.0, "warning" => 0.5, _ => 0.0
        },
        report.disrupted_channels as f64 / report.total_channels.max(1) as f64,
        (report.aggregate_power / 100.0).min(1.0),
        (report.power_rate_of_change / 50.0).min(1.0).max(-1.0),
        report.signal_disruption / 100.0,
        0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once(history: &mut SignalHistory) -> Result<ProximityReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    // Ensure enough channel state slots
    while history.channels.len() < samples.len() {
        history.channels.push(ChannelState::new());
    }

    let mut disrupted = 0usize;
    let mut aggregate_power = 0.0f64;
    let mut total_disruption = 0.0f64;

    for (i, sample) in samples.iter().enumerate() {
        let val = sample.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let abs_val = val.abs();
        aggregate_power += abs_val * abs_val;

        if i < history.channels.len() {
            if history.channels[i].is_disrupted(val) {
                disrupted += 1;
                let std_dev = history.channels[i].variance().sqrt().max(0.1);
                total_disruption += (val - history.channels[i].mean).abs() / std_dev;
            }
            history.channels[i].update(val);
        }
    }

    aggregate_power = aggregate_power.sqrt();
    let power_roc = aggregate_power - history.prev_aggregate_power;
    history.prev_aggregate_power = aggregate_power;

    // Forklift detection: multiple channels disrupted simultaneously + rapid power increase
    let forklift_detected = disrupted >= MIN_CHANNELS_DISRUPTED
        && (total_disruption > DISRUPTION_THRESHOLD || power_roc > RATE_THRESHOLD);

    let proximity_level = if forklift_detected && power_roc > RATE_THRESHOLD * 2.0 {
        "danger"
    } else if forklift_detected {
        "warning"
    } else {
        "safe"
    };

    let alert = forklift_detected && history.alert_cooldown == 0;
    if alert {
        history.alert_cooldown = 5; // Cooldown to avoid alert spam
    }
    if history.alert_cooldown > 0 {
        history.alert_cooldown -= 1;
    }

    Ok(ProximityReport {
        forklift_detected,
        proximity_level: proximity_level.to_string(),
        disrupted_channels: disrupted,
        total_channels: samples.len(),
        aggregate_power,
        power_rate_of_change: power_roc,
        signal_disruption: total_disruption,
        alert,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);

    eprintln!("[cog-forklift-proximity] starting (interval={interval}s)");
    let mut history = SignalHistory::new();

    loop {
        let start = Instant::now();
        match run_once(&mut history) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if let Err(e) = store_detection(&report) {
                    eprintln!("[cog-forklift-proximity] store error: {e}");
                }
                if report.alert {
                    eprintln!("[cog-forklift-proximity] ALERT: Forklift detected! Level: {}", report.proximity_level);
                }
            }
            Err(e) => eprintln!("[cog-forklift-proximity] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
