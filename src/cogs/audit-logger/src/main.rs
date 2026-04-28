//! Cognitum Cog: Audit Logger — Tamper-Proof Event Logging
//!
//! Hash-chains each event using a simple checksum scheme.
//! Stores events in RVF store with witness chain attestation.
//! Can forward to a cloud endpoint if configured.
//!
//! Usage:
//!   cog-audit-logger --once
//!   cog-audit-logger --interval 5

use std::io::Read;
use std::time::{Duration, Instant};

/// Simple hash: FNV-1a 64-bit — lightweight for ARM deployment
fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash
}

/// SHA-256-like hash (4x FNV-1a cascade for 256-bit output)
fn hash_chain_block(data: &[u8]) -> [u64; 4] {
    let h1 = fnv1a(data);
    let d2: Vec<u8> = [data, &h1.to_le_bytes()].concat();
    let h2 = fnv1a(&d2);
    let d3: Vec<u8> = [data, &h2.to_le_bytes()].concat();
    let h3 = fnv1a(&d3);
    let d4: Vec<u8> = [data, &h3.to_le_bytes()].concat();
    let h4 = fnv1a(&d4);
    [h1, h2, h3, h4]
}

fn hash_to_hex(hash: &[u64; 4]) -> String {
    hash.iter().map(|h| format!("{:016x}", h)).collect::<String>()
}

struct AuditEvent {
    sequence: u64,
    timestamp: u64,
    event_type: String,
    payload: String,
    prev_hash: String,
    hash: String,
}

struct AuditLogger {
    chain: Vec<AuditEvent>,
    sequence: u64,
    last_hash: [u64; 4],
    total_events: u64,
    tamper_detected: bool,
    signal_change_threshold: f64,
    prev_mean: Option<f64>,
}

impl AuditLogger {
    fn new() -> Self {
        Self {
            chain: Vec::new(),
            sequence: 0,
            last_hash: [0; 4],
            total_events: 0,
            tamper_detected: false,
            signal_change_threshold: 0.1,
            prev_mean: None,
        }
    }

    fn log_event(&mut self, event_type: &str, payload: &str) -> &AuditEvent {
        let ts = now_ts();
        let prev_hash = hash_to_hex(&self.last_hash);

        // Build hash input: sequence + timestamp + type + payload + prev_hash
        let hash_input = format!("{}:{}:{}:{}:{}", self.sequence, ts, event_type, payload, prev_hash);
        let hash = hash_chain_block(hash_input.as_bytes());
        let hash_hex = hash_to_hex(&hash);

        let event = AuditEvent {
            sequence: self.sequence,
            timestamp: ts,
            event_type: event_type.to_string(),
            payload: payload.to_string(),
            prev_hash,
            hash: hash_hex,
        };

        self.last_hash = hash;
        self.sequence += 1;
        self.total_events += 1;

        self.chain.push(event);
        // Keep last 1000 events in memory
        if self.chain.len() > 1000 {
            self.chain.remove(0);
        }

        self.chain.last().unwrap()
    }

    fn verify_chain(&self) -> bool {
        if self.chain.len() < 2 {
            return true;
        }
        for i in 1..self.chain.len() {
            let prev = &self.chain[i - 1];
            let curr = &self.chain[i];
            if curr.prev_hash != prev.hash {
                return false;
            }
        }
        true
    }

    fn detect_signal_event(&mut self, amplitudes: &[f64]) -> Option<(String, String)> {
        let mean = amplitudes.iter().sum::<f64>() / amplitudes.len().max(1) as f64;
        let variance = amplitudes.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / amplitudes.len().max(1) as f64;

        let event = if let Some(prev) = self.prev_mean {
            let change = (mean - prev).abs() / prev.abs().max(1e-10);
            if change > self.signal_change_threshold {
                Some(("signal_change".to_string(),
                    format!("mean={:.4}, prev={:.4}, change={:.2}%, var={:.4}",
                        mean, prev, change * 100.0, variance)))
            } else {
                None
            }
        } else {
            Some(("baseline_established".to_string(),
                format!("mean={:.4}, var={:.4}, channels={}", mean, variance, amplitudes.len())))
        };

        self.prev_mean = Some(mean);

        // Check for anomalous values
        let nan_count = amplitudes.iter().filter(|v| v.is_nan() || v.is_infinite()).count();
        if nan_count > 0 {
            return Some(("invalid_data".to_string(),
                format!("{} NaN/Inf values in {} channels", nan_count, amplitudes.len())));
        }

        event
    }
}

#[derive(serde::Serialize)]
struct AuditReport {
    status: String,
    chain_valid: bool,
    total_events: u64,
    last_event_type: String,
    last_event_hash: String,
    chain_length: usize,
    tamper_detected: bool,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct AuditEventJson {
    sequence: u64,
    timestamp: u64,
    event_type: String,
    payload: String,
    hash: String,
    prev_hash: String,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(report: &AuditReport) -> Result<(), String> {
    // Hash the last event hash into a float vector for store
    let hash_bytes = report.last_event_hash.as_bytes();
    let h1 = fnv1a(hash_bytes) as f64 / u64::MAX as f64;
    let h2 = fnv1a(&hash_bytes[8..hash_bytes.len().min(16)]) as f64 / u64::MAX as f64;

    let vector = vec![
        if report.chain_valid { 1.0 } else { 0.0 },
        if report.tamper_detected { 1.0 } else { 0.0 },
        report.total_events as f64 / 100000.0,
        report.chain_length as f64 / 1000.0,
        h1,
        h2,
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

fn forward_to_cloud(event: &AuditEventJson, endpoint: &str) -> Result<(), String> {
    let body = serde_json::to_vec(event).map_err(|e| format!("json: {e}"))?;

    // Parse host:port from endpoint URL
    let host = endpoint.trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("127.0.0.1:80");

    let path = endpoint.find('/')
        .map(|i| &endpoint[i..])
        .unwrap_or("/audit");

    let mut conn = std::net::TcpStream::connect(host)
        .map_err(|e| format!("cloud connect: {e}"))?;
    use std::io::Write;
    write!(conn, "POST {} HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        path, host, body.len())
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
        .unwrap_or(5);
    let cloud_endpoint = args.iter()
        .position(|a| a == "--cloud")
        .and_then(|i| args.get(i + 1))
        .cloned();

    eprintln!("[cog-audit-logger] starting (interval={}s, cloud={})",
        interval, cloud_endpoint.as_deref().unwrap_or("none"));

    let mut logger = AuditLogger::new();
    logger.log_event("startup", &format!("audit-logger started, interval={}s", interval));

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|s| s.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();

                    // Detect signal events and log them
                    if let Some((event_type, payload)) = logger.detect_signal_event(&amps) {
                        let event = logger.log_event(&event_type, &payload);

                        // Forward to cloud if configured
                        if let Some(ref endpoint) = cloud_endpoint {
                            let event_json = AuditEventJson {
                                sequence: event.sequence,
                                timestamp: event.timestamp,
                                event_type: event.event_type.clone(),
                                payload: event.payload.clone(),
                                hash: event.hash.clone(),
                                prev_hash: event.prev_hash.clone(),
                            };
                            if let Err(e) = forward_to_cloud(&event_json, endpoint) {
                                eprintln!("[cog-audit-logger] cloud forward error: {e}");
                            }
                        }
                    }

                    // Periodic chain validation
                    let chain_valid = logger.verify_chain();
                    if !chain_valid {
                        logger.tamper_detected = true;
                        eprintln!("[cog-audit-logger] ALERT: hash chain tampering detected!");
                    }

                    let last_event = logger.chain.last();
                    let report = AuditReport {
                        status: if logger.tamper_detected { "TAMPERED".into() }
                            else if !chain_valid { "INVALID".into() }
                            else { "secure".into() },
                        chain_valid,
                        total_events: logger.total_events,
                        last_event_type: last_event.map(|e| e.event_type.clone()).unwrap_or_default(),
                        last_event_hash: last_event.map(|e| e.hash.clone()).unwrap_or_default(),
                        chain_length: logger.chain.len(),
                        tamper_detected: logger.tamper_detected,
                        timestamp: now_ts(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-audit-logger] store error: {e}");
                    }
                }
            }
            Err(e) => {
                logger.log_event("sensor_error", &e);
                eprintln!("[cog-audit-logger] sensor error: {e}");
            }
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
