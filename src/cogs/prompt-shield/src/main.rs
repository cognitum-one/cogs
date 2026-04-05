//! Cognitum Cog: Prompt Shield — Signal Replay/Injection Attack Detector
//!
//! Stores fingerprints of recent signal patterns, detects exact/near
//! duplicates (cosine similarity >0.99) indicating replay attacks.
//! Also detects out-of-range values indicating injection.
//!
//! Usage:
//!   cog-prompt-shield --once
//!   cog-prompt-shield --interval 2

use std::io::Read;
use std::time::{Duration, Instant};

const FINGERPRINT_CAPACITY: usize = 200;
const REPLAY_THRESHOLD: f64 = 0.99;
const NEAR_REPLAY_THRESHOLD: f64 = 0.97;

struct Fingerprint {
    vector: Vec<f64>,
    timestamp: u64,
}

struct PromptShield {
    fingerprints: Vec<Fingerprint>,
    value_min: f64,
    value_max: f64,
    range_learned: bool,
    range_samples: u64,
    range_learning_count: u64,
    running_min: f64,
    running_max: f64,
    total_replays: u64,
    total_injections: u64,
    total_anomalies: u64,
}

impl PromptShield {
    fn new() -> Self {
        Self {
            fingerprints: Vec::with_capacity(FINGERPRINT_CAPACITY),
            value_min: f64::MAX,
            value_max: f64::MIN,
            range_learned: false,
            range_samples: 0,
            range_learning_count: 100,
            running_min: f64::MAX,
            running_max: f64::MIN,
            total_replays: 0,
            total_injections: 0,
            total_anomalies: 0,
        }
    }

    fn process(&mut self, amplitudes: &[f64]) -> ShieldResult {
        let ts = now_ts();

        // Learn value range
        if !self.range_learned {
            for &v in amplitudes {
                if v < self.running_min { self.running_min = v; }
                if v > self.running_max { self.running_max = v; }
            }
            self.range_samples += 1;
            if self.range_samples >= self.range_learning_count {
                let margin = (self.running_max - self.running_min) * 0.2;
                self.value_min = self.running_min - margin;
                self.value_max = self.running_max + margin;
                self.range_learned = true;
            }
            return ShieldResult::Learning {
                progress_pct: (self.range_samples as f64 / self.range_learning_count as f64 * 100.0).min(100.0),
            };
        }

        let mut threats = Vec::new();

        // Check 1: Out-of-range values (injection)
        let out_of_range: Vec<usize> = amplitudes.iter().enumerate()
            .filter(|(_, &v)| v < self.value_min || v > self.value_max)
            .map(|(i, _)| i)
            .collect();

        if !out_of_range.is_empty() {
            self.total_injections += 1;
            threats.push(Threat {
                threat_type: "injection".into(),
                confidence: (out_of_range.len() as f64 / amplitudes.len() as f64).min(1.0),
                detail: format!("{} channels out of range [{:.2}, {:.2}]",
                    out_of_range.len(), self.value_min, self.value_max),
            });
        }

        // Check 2: NaN/Inf values (malformed input)
        let invalid_count = amplitudes.iter().filter(|v| v.is_nan() || v.is_infinite()).count();
        if invalid_count > 0 {
            self.total_anomalies += 1;
            threats.push(Threat {
                threat_type: "malformed".into(),
                confidence: 1.0,
                detail: format!("{} NaN/Inf values detected", invalid_count),
            });
        }

        // Check 3: Replay detection via fingerprint comparison
        let norm = normalize(amplitudes);
        let mut max_similarity = 0.0_f64;
        let mut replay_age_secs = 0u64;

        for fp in &self.fingerprints {
            let sim = cosine_similarity(&norm, &fp.vector);
            if sim > max_similarity {
                max_similarity = sim;
                replay_age_secs = ts.saturating_sub(fp.timestamp);
            }
        }

        let is_replay = max_similarity > REPLAY_THRESHOLD && replay_age_secs > 0;
        let is_near_replay = max_similarity > NEAR_REPLAY_THRESHOLD && replay_age_secs > 2;

        if is_replay {
            self.total_replays += 1;
            threats.push(Threat {
                threat_type: "replay".into(),
                confidence: max_similarity,
                detail: format!("exact replay (similarity={:.4}, age={}s)", max_similarity, replay_age_secs),
            });
        } else if is_near_replay {
            threats.push(Threat {
                threat_type: "near_replay".into(),
                confidence: (max_similarity - NEAR_REPLAY_THRESHOLD) / (REPLAY_THRESHOLD - NEAR_REPLAY_THRESHOLD),
                detail: format!("near replay (similarity={:.4}, age={}s)", max_similarity, replay_age_secs),
            });
        }

        // Check 4: Zero-variance signal (constant injection)
        let mean = amplitudes.iter().sum::<f64>() / amplitudes.len().max(1) as f64;
        let variance = amplitudes.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / amplitudes.len().max(1) as f64;
        if variance < 1e-12 && amplitudes.len() > 2 {
            self.total_anomalies += 1;
            threats.push(Threat {
                threat_type: "constant_signal".into(),
                confidence: 0.9,
                detail: "zero variance — possible synthetic signal".into(),
            });
        }

        // Store fingerprint
        self.fingerprints.push(Fingerprint { vector: norm, timestamp: ts });
        if self.fingerprints.len() > FINGERPRINT_CAPACITY {
            self.fingerprints.remove(0);
        }

        ShieldResult::Active {
            threats,
            max_similarity,
            replay_age_secs,
        }
    }
}

fn normalize(v: &[f64]) -> Vec<f64> {
    let mag = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag < 1e-12 {
        vec![0.0; v.len()]
    } else {
        v.iter().map(|x| x / mag).collect()
    }
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 { return 0.0; }
    let dot: f64 = a.iter().take(n).zip(b.iter().take(n)).map(|(x, y)| x * y).sum();
    let mag_a = a.iter().take(n).map(|x| x * x).sum::<f64>().sqrt();
    let mag_b = b.iter().take(n).map(|x| x * x).sum::<f64>().sqrt();
    if mag_a < 1e-12 || mag_b < 1e-12 { 0.0 } else { dot / (mag_a * mag_b) }
}

struct Threat {
    threat_type: String,
    confidence: f64,
    detail: String,
}

enum ShieldResult {
    Learning { progress_pct: f64 },
    Active {
        threats: Vec<Threat>,
        max_similarity: f64,
        replay_age_secs: u64,
    },
}

#[derive(serde::Serialize)]
struct ShieldReport {
    status: String,
    threat_detected: bool,
    threat_count: usize,
    threats: Vec<ThreatJson>,
    max_similarity: f64,
    total_replays: u64,
    total_injections: u64,
    total_anomalies: u64,
    fingerprint_count: usize,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct ThreatJson {
    threat_type: String,
    confidence: f64,
    detail: String,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

fn store_vector(report: &ShieldReport) -> Result<(), String> {
    let max_conf = report.threats.iter()
        .map(|t| t.confidence)
        .fold(0.0_f64, f64::max);
    let vector = vec![
        if report.threat_detected { 1.0 } else { 0.0 },
        max_conf,
        report.max_similarity,
        report.total_replays as f64 / 100.0,
        report.total_injections as f64 / 100.0,
        report.total_anomalies as f64 / 100.0,
        report.threat_count as f64 / 10.0,
        report.fingerprint_count as f64 / FINGERPRINT_CAPACITY as f64,
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
        .unwrap_or(2);

    eprintln!("[cog-prompt-shield] starting (interval={}s)", interval);
    let mut shield = PromptShield::new();

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|s| s.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256)
                        .filter_map(|ch| ch.get("value").and_then(|v| v.as_f64()))
                        .collect();
                    if amps.is_empty() { continue; }

                    let result = shield.process(&amps);
                    let report = match result {
                        ShieldResult::Learning { progress_pct } => ShieldReport {
                            status: format!("learning ({:.0}%)", progress_pct),
                            threat_detected: false, threat_count: 0,
                            threats: Vec::new(), max_similarity: 0.0,
                            total_replays: 0, total_injections: 0, total_anomalies: 0,
                            fingerprint_count: shield.fingerprints.len(),
                            timestamp: now_ts(),
                        },
                        ShieldResult::Active { threats, max_similarity, replay_age_secs } => {
                            let threat_detected = !threats.is_empty();
                            let threat_jsons: Vec<ThreatJson> = threats.iter().map(|t| ThreatJson {
                                threat_type: t.threat_type.clone(),
                                confidence: t.confidence,
                                detail: t.detail.clone(),
                            }).collect();
                            ShieldReport {
                                status: if threat_detected { "THREAT".into() } else { "secure".into() },
                                threat_detected,
                                threat_count: threat_jsons.len(),
                                threats: threat_jsons,
                                max_similarity,
                                total_replays: shield.total_replays,
                                total_injections: shield.total_injections,
                                total_anomalies: shield.total_anomalies,
                                fingerprint_count: shield.fingerprints.len(),
                                timestamp: now_ts(),
                            }
                        }
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-prompt-shield] store error: {e}");
                    }
                    if report.threat_detected {
                        for t in &report.threats {
                            eprintln!("[cog-prompt-shield] THREAT: {} — {} (confidence={:.0}%)",
                                t.threat_type, t.detail, t.confidence * 100.0);
                        }
                    }
                }
            }
            Err(e) => eprintln!("[cog-prompt-shield] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
