//! Cognitum Cog: Tailgating Detection
//!
//! Detects two people passing through a doorway by monitoring for
//! double-presence pattern: first presence spike, brief gap, second
//! spike within 5-10s window. Counts entries vs expected badge events.
//!
//! Usage:
//!   cog-tailgating --once
//!   cog-tailgating --interval 1

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

#[derive(Clone, Copy, PartialEq)]
enum PresenceState {
    Empty,
    FirstPresence,
    Gap,
    SecondPresence,
}

struct TailgateDetector {
    baseline: WelfordStats,
    presence_threshold: f64,
    state: PresenceState,
    first_spike_time: Option<Instant>,
    gap_start_time: Option<Instant>,
    second_spike_time: Option<Instant>,
    tailgate_window_secs: f64,
    min_gap_ms: u64,
    max_gap_ms: u64,
    entry_count: u64,
    tailgate_count: u64,
    badge_events: u64,
    learning_samples: u64,
    consecutive_presence: u32,
    presence_required: u32,
}

impl TailgateDetector {
    fn new() -> Self {
        Self {
            baseline: WelfordStats::new(),
            presence_threshold: 2.5,
            state: PresenceState::Empty,
            first_spike_time: None,
            gap_start_time: None,
            second_spike_time: None,
            tailgate_window_secs: 10.0,
            min_gap_ms: 300,
            max_gap_ms: 5000,
            entry_count: 0,
            tailgate_count: 0,
            badge_events: 0,
            learning_samples: 40,
            consecutive_presence: 0,
            presence_required: 2,
        }
    }

    fn process(&mut self, signal_energy: f64) -> TailgateResult {
        if self.baseline.count < self.learning_samples {
            self.baseline.update(signal_energy);
            return TailgateResult::Learning {
                progress_pct: (self.baseline.count as f64 / self.learning_samples as f64 * 100.0).min(100.0),
            };
        }

        let z = self.baseline.z_score(signal_energy);
        let is_presence = z.abs() > self.presence_threshold;
        let now = Instant::now();

        match self.state {
            PresenceState::Empty => {
                if is_presence {
                    self.consecutive_presence += 1;
                    if self.consecutive_presence >= self.presence_required {
                        self.state = PresenceState::FirstPresence;
                        self.first_spike_time = Some(now);
                        self.entry_count += 1;
                        self.consecutive_presence = 0;
                        return TailgateResult::FirstEntry { z_score: z };
                    }
                } else {
                    self.consecutive_presence = 0;
                    self.baseline.update(signal_energy); // adapt
                }
                TailgateResult::Clear
            }
            PresenceState::FirstPresence => {
                if !is_presence {
                    self.state = PresenceState::Gap;
                    self.gap_start_time = Some(now);
                } else if let Some(first) = self.first_spike_time {
                    if first.elapsed().as_secs_f64() > self.tailgate_window_secs {
                        // Single person, too long — reset
                        self.state = PresenceState::Empty;
                    }
                }
                TailgateResult::Monitoring { state: "first_presence" }
            }
            PresenceState::Gap => {
                if let Some(gap_start) = self.gap_start_time {
                    let gap_ms = gap_start.elapsed().as_millis() as u64;

                    if is_presence && gap_ms >= self.min_gap_ms && gap_ms <= self.max_gap_ms {
                        // Second person detected!
                        self.state = PresenceState::SecondPresence;
                        self.second_spike_time = Some(now);
                        self.entry_count += 1;
                        self.tailgate_count += 1;

                        let confidence = self.compute_confidence(z, gap_ms);
                        self.state = PresenceState::Empty;

                        return TailgateResult::TailgateDetected {
                            confidence,
                            gap_ms,
                            z_score: z,
                            entries: self.entry_count,
                        };
                    } else if gap_ms > self.max_gap_ms {
                        // Gap too long — single person entry
                        self.state = PresenceState::Empty;
                    } else if is_presence && gap_ms < self.min_gap_ms {
                        // Gap too short — same person still passing
                        self.state = PresenceState::FirstPresence;
                    }
                }
                TailgateResult::Monitoring { state: "gap" }
            }
            PresenceState::SecondPresence => {
                // Transition handled inline above; fallback reset
                self.state = PresenceState::Empty;
                TailgateResult::Clear
            }
        }
    }

    fn compute_confidence(&self, z_score: f64, gap_ms: u64) -> f64 {
        // Confidence based on z-score strength and gap timing
        let z_factor = (z_score.abs() / 5.0).min(1.0);
        let ideal_gap = (self.min_gap_ms + self.max_gap_ms) / 2;
        let gap_factor = 1.0 - ((gap_ms as f64 - ideal_gap as f64).abs() / ideal_gap as f64).min(1.0);
        (z_factor * 0.6 + gap_factor * 0.4).min(1.0)
    }

    fn register_badge(&mut self) {
        self.badge_events += 1;
    }

    fn mismatch_count(&self) -> i64 {
        self.entry_count as i64 - self.badge_events as i64
    }
}

enum TailgateResult {
    Learning { progress_pct: f64 },
    Clear,
    FirstEntry { z_score: f64 },
    Monitoring { state: &'static str },
    TailgateDetected { confidence: f64, gap_ms: u64, z_score: f64, entries: u64 },
}

#[derive(serde::Serialize)]
struct TailgateReport {
    status: String,
    tailgate_detected: bool,
    confidence: f64,
    gap_ms: u64,
    entry_count: u64,
    tailgate_count: u64,
    badge_events: u64,
    entry_badge_mismatch: i64,
    timestamp: u64,
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

fn store_vector(report: &TailgateReport) -> Result<(), String> {
    let vector = vec![
        if report.tailgate_detected { 1.0 } else { 0.0 },
        report.confidence,
        report.gap_ms as f64 / 10000.0,
        report.entry_count as f64 / 1000.0,
        report.tailgate_count as f64 / 100.0,
        report.entry_badge_mismatch.abs() as f64 / 100.0,
        report.badge_events as f64 / 1000.0,
        0.0,
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
        .unwrap_or(1);

    eprintln!("[cog-tailgating] starting (interval={}s)", interval);
    let mut detector = TailgateDetector::new();

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

                    // Signal energy: sum of squared amplitudes
                    let energy = amps.iter().map(|v| v * v).sum::<f64>() / amps.len() as f64;
                    let result = detector.process(energy);

                    let (status, tailgate, conf, gap) = match result {
                        TailgateResult::Learning { progress_pct } =>
                            (format!("learning ({:.0}%)", progress_pct), false, 0.0, 0),
                        TailgateResult::Clear =>
                            ("clear".into(), false, 0.0, 0),
                        TailgateResult::FirstEntry { .. } =>
                            ("entry_detected".into(), false, 0.0, 0),
                        TailgateResult::Monitoring { state } =>
                            (format!("monitoring ({})", state), false, 0.0, 0),
                        TailgateResult::TailgateDetected { confidence, gap_ms, .. } =>
                            ("TAILGATE".into(), true, confidence, gap_ms),
                    };

                    let report = TailgateReport {
                        status,
                        tailgate_detected: tailgate,
                        confidence: conf,
                        gap_ms: gap,
                        entry_count: detector.entry_count,
                        tailgate_count: detector.tailgate_count,
                        badge_events: detector.badge_events,
                        entry_badge_mismatch: detector.mismatch_count(),
                        timestamp: now_ts(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_vector(&report) {
                        eprintln!("[cog-tailgating] store error: {e}");
                    }
                    if tailgate {
                        eprintln!("[cog-tailgating] ALERT: tailgating detected (confidence={:.0}%, gap={}ms)",
                            conf * 100.0, gap);
                    }
                }
            }
            Err(e) => eprintln!("[cog-tailgating] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
