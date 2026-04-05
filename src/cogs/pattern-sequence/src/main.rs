//! Cognitum Cog: Pattern Sequence
//!
//! Detect daily routines. Record hourly activity patterns. Compare today
//! vs historical average. Report routine adherence score and deviations.
//!
//! Usage:
//!   cog-pattern-sequence --once
//!   cog-pattern-sequence --interval 60

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;
const HOURS: usize = 24;

/// Running hourly statistics
struct HourlyStats {
    count: [u64; HOURS],
    mean: [f64; HOURS],
    m2: [f64; HOURS],
}

impl HourlyStats {
    fn new() -> Self {
        Self {
            count: [0; HOURS],
            mean: [0.0; HOURS],
            m2: [0.0; HOURS],
        }
    }

    fn update(&mut self, hour: usize, value: f64) {
        if hour >= HOURS { return; }
        self.count[hour] += 1;
        let n = self.count[hour] as f64;
        let delta = value - self.mean[hour];
        self.mean[hour] += delta / n;
        let delta2 = value - self.mean[hour];
        self.m2[hour] += delta * delta2;
    }

    fn std_dev(&self, hour: usize) -> f64 {
        if hour >= HOURS || self.count[hour] < 2 { return 1.0; }
        (self.m2[hour] / (self.count[hour] - 1) as f64).sqrt().max(0.01)
    }

    fn z_score(&self, hour: usize, value: f64) -> f64 {
        if hour >= HOURS { return 0.0; }
        (value - self.mean[hour]) / self.std_dev(hour)
    }

    /// Adherence score: how close current value is to historical mean
    fn adherence(&self, hour: usize, value: f64) -> f64 {
        let z = self.z_score(hour, value).abs();
        (-z * z / 2.0).exp() // Gaussian-like score [0, 1]
    }
}

/// Compute activity level from sensor values
fn activity_level(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let derivatives: Vec<f64> = values.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    derivatives.iter().sum::<f64>() / derivatives.len() as f64
}

#[derive(serde::Serialize)]
struct SequenceResult {
    current_hour: usize,
    activity_level: f64,
    adherence_score: f64,
    z_score: f64,
    hourly_means: Vec<f64>,
    deviations: Vec<String>,
    routine_status: String,
    observation_days: u64,
    vector: [f64; DIM],
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
    let start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[start..]).map_err(|e| format!("parse: {e}"))
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

fn run_once(stats: &mut HourlyStats) -> Result<SequenceResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;
    let values: Vec<f64> = samples.iter().filter_map(|s| s.get("value").and_then(|v| v.as_f64())).collect();
    if values.is_empty() { return Err("no sensor readings".into()); }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let hour = ((now / 3600) % 24) as usize;

    let activity = activity_level(&values);
    let adherence = stats.adherence(hour, activity);
    let z = stats.z_score(hour, activity);

    // Update hourly statistics
    stats.update(hour, activity);

    let mut deviations = Vec::new();
    if z.abs() > 2.0 {
        let dir = if z > 0.0 { "above" } else { "below" };
        deviations.push(format!("HOUR_{hour}: activity {dir} normal (z={z:.2})"));
    }

    let routine_status = if adherence > 0.8 {
        "on_routine"
    } else if adherence > 0.5 {
        "minor_deviation"
    } else if adherence > 0.2 {
        "significant_deviation"
    } else {
        "routine_break"
    };

    let hourly_means: Vec<f64> = (0..HOURS).map(|h| stats.mean[h]).collect();
    let obs_days = stats.count[hour];

    let vector = [
        activity,
        adherence,
        z / 4.0,
        hour as f64 / 24.0,
        stats.mean[hour],
        stats.std_dev(hour),
        obs_days as f64 / 30.0,
        if deviations.is_empty() { 0.0 } else { 1.0 },
    ];

    let _ = store_vector(&vector);

    Ok(SequenceResult {
        current_hour: hour,
        activity_level: activity,
        adherence_score: adherence,
        z_score: z,
        hourly_means,
        deviations,
        routine_status: routine_status.into(),
        observation_days: obs_days,
        vector,
        timestamp: now,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);

    eprintln!("[cog-pattern-sequence] starting (interval={interval}s, once={once})");

    let mut stats = HourlyStats::new();

    loop {
        let start = Instant::now();
        match run_once(&mut stats) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.deviations.is_empty() {
                    eprintln!("[cog-pattern-sequence] ALERT: {:?}", r.deviations);
                }
            }
            Err(e) => eprintln!("[cog-pattern-sequence] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
