//! Cognitum Cog: Temporal Logic
//!
//! Safety rule enforcement using temporal logic (LTL-like).
//! Define rules: "if presence then breathing must be detected within 30s".
//! Monitor rule satisfaction in real-time.
//!
//! Usage:
//!   cog-temporal-logic --once
//!   cog-temporal-logic --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// Temporal logic proposition
#[derive(Clone)]
enum Prop {
    /// Channel value above threshold
    Above(String, f64),
    /// Channel value below threshold
    Below(String, f64),
    /// Channel is active (non-zero recently)
    Active(String),
}

impl Prop {
    fn evaluate(&self, channels: &std::collections::HashMap<String, Vec<f64>>) -> bool {
        match self {
            Prop::Above(ch, thresh) => channels.get(ch)
                .and_then(|v| v.last())
                .map(|&v| v > *thresh)
                .unwrap_or(false),
            Prop::Below(ch, thresh) => channels.get(ch)
                .and_then(|v| v.last())
                .map(|&v| v < *thresh)
                .unwrap_or(false),
            Prop::Active(ch) => channels.get(ch)
                .map(|v| v.iter().any(|&x| x.abs() > 0.01))
                .unwrap_or(false),
        }
    }
}

/// LTL-like temporal rule
struct TemporalRule {
    name: String,
    /// Trigger condition
    trigger: Prop,
    /// Required response
    response: Prop,
    /// Time window in seconds
    deadline_secs: u64,
    /// When the trigger was first observed (epoch secs)
    trigger_time: Option<u64>,
    /// Whether response has been observed
    responded: bool,
    /// Total checks
    total_checks: u64,
    /// Violations
    violations: u64,
}

impl TemporalRule {
    fn new(name: &str, trigger: Prop, response: Prop, deadline_secs: u64) -> Self {
        Self {
            name: name.into(),
            trigger,
            response,
            deadline_secs,
            trigger_time: None,
            responded: false,
            total_checks: 0,
            violations: 0,
        }
    }

    fn check(&mut self, channels: &std::collections::HashMap<String, Vec<f64>>, now: u64) -> Option<String> {
        self.total_checks += 1;
        let triggered = self.trigger.evaluate(channels);
        let response_met = self.response.evaluate(channels);

        if triggered && self.trigger_time.is_none() {
            self.trigger_time = Some(now);
            self.responded = false;
        }

        if response_met {
            self.responded = true;
        }

        if let Some(t) = self.trigger_time {
            if now - t > self.deadline_secs && !self.responded {
                self.violations += 1;
                self.trigger_time = None; // Reset for next check
                return Some(format!("VIOLATION: {} — deadline {}s exceeded", self.name, self.deadline_secs));
            }
            if self.responded {
                self.trigger_time = None; // Rule satisfied, reset
            }
        }

        // Reset trigger tracking if condition no longer active
        if !triggered && self.trigger_time.is_some() && self.responded {
            self.trigger_time = None;
        }

        None
    }

    fn satisfaction_rate(&self) -> f64 {
        if self.total_checks == 0 { return 1.0; }
        1.0 - self.violations as f64 / self.total_checks as f64
    }
}

#[derive(serde::Serialize)]
struct TemporalResult {
    rules_checked: usize,
    violations: Vec<String>,
    satisfaction_rates: Vec<(String, f64)>,
    overall_safety: f64,
    safety_status: String,
    vector: [f64; DIM],
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
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

fn run_once(rules: &mut Vec<TemporalRule>) -> Result<TemporalResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    let mut channels: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0").to_string();
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        channels.entry(ch).or_default().push(val);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    let mut violations = Vec::new();
    for rule in rules.iter_mut() {
        if let Some(v) = rule.check(&channels, now) {
            violations.push(v);
        }
    }

    let rates: Vec<(String, f64)> = rules.iter()
        .map(|r| (r.name.clone(), r.satisfaction_rate()))
        .collect();
    let overall = rates.iter().map(|r| r.1).sum::<f64>() / rates.len().max(1) as f64;

    let safety_status = if overall >= 0.99 {
        "safe"
    } else if overall >= 0.95 {
        "warning"
    } else if overall >= 0.8 {
        "degraded"
    } else {
        "unsafe"
    };

    let vector = [
        overall,
        violations.len() as f64 / 10.0,
        rules.len() as f64 / 10.0,
        rates.get(0).map(|r| r.1).unwrap_or(1.0),
        rates.get(1).map(|r| r.1).unwrap_or(1.0),
        rates.get(2).map(|r| r.1).unwrap_or(1.0),
        if safety_status == "unsafe" { 1.0 } else { 0.0 },
        now as f64 % 86400.0 / 86400.0, // Time of day fraction
    ];

    let _ = store_vector(&vector);

    Ok(TemporalResult {
        rules_checked: rules.len(),
        violations,
        satisfaction_rates: rates,
        overall_safety: overall,
        safety_status: safety_status.into(),
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
        .unwrap_or(5);

    eprintln!("[cog-temporal-logic] starting (interval={interval}s, once={once})");

    // Define safety rules
    let mut rules = vec![
        TemporalRule::new(
            "presence_requires_breathing",
            Prop::Active("ch0".into()),
            Prop::Above("ch1".into(), 0.01),
            30,
        ),
        TemporalRule::new(
            "motion_requires_heartbeat",
            Prop::Above("ch0".into(), 0.3),
            Prop::Active("ch1".into()),
            15,
        ),
        TemporalRule::new(
            "high_activity_below_ceiling",
            Prop::Above("ch0".into(), 0.8),
            Prop::Below("ch0".into(), 1.5),
            10,
        ),
    ];

    loop {
        let start = Instant::now();
        match run_once(&mut rules) {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if !r.violations.is_empty() {
                    eprintln!("[cog-temporal-logic] ALERT: {:?}", r.violations);
                }
            }
            Err(e) => eprintln!("[cog-temporal-logic] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
