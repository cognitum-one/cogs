//! Cognitum Cog: Fall Detection
//!
//! Two-stage impact + stillness detector over the ambient feature stream.
//! ADR-002. Implements ADR-001 cog-as-plugin contract.
//!
//! Usage:
//!   cog-fall-detect --once
//!   cog-fall-detect --interval 1 --impact-threshold 6.0 --stillness-window 8
//!   cog-fall-detect --ruview-mode

use std::io::{Read, Write};
use std::time::{Duration, Instant};

struct Welford {
    count: u64,
    mean: f64,
    m2: f64,
}

impl Welford {
    fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, v: f64) {
        self.count += 1;
        let d = v - self.mean;
        self.mean += d / self.count as f64;
        self.m2 += d * (v - self.mean);
    }
    fn std_dev(&self) -> f64 {
        if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() }
    }
    fn z(&self, v: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 { 0.0 } else { (v - self.mean) / sd }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State { Quiet, Monitoring { until: Instant, stillness_frames: u32 }, Cooldown { until: Instant } }

struct FallDetector {
    state: State,
    amp_baseline: Welford,
    var_baseline: Welford,
    impact_threshold: f64,
    stillness_window_secs: u64,
    cooldown_secs: u64,
    quiet_threshold_var: f64,
    total_falls: u64,
    last_impact_z: f64,
    ruview_mode: bool,
    head_height_baseline: Welford,
}

impl FallDetector {
    fn new(impact_threshold: f64, stillness_window_secs: u64, cooldown_secs: u64, ruview_mode: bool) -> Self {
        Self {
            state: State::Quiet,
            amp_baseline: Welford::new(),
            var_baseline: Welford::new(),
            impact_threshold,
            stillness_window_secs,
            cooldown_secs,
            quiet_threshold_var: 0.001,
            total_falls: 0,
            last_impact_z: 0.0,
            ruview_mode,
            head_height_baseline: Welford::new(),
        }
    }

    fn update(&mut self, mean_amp: f64, var: f64, head_height_proxy: Option<f64>) -> Report {
        // Update baselines (running, with bias toward quiet frames)
        let z_var = self.var_baseline.z(var);
        let is_quiet = z_var.abs() < 1.5 && var < self.quiet_threshold_var.max(self.var_baseline.mean * 3.0);
        if is_quiet || self.amp_baseline.count < 30 {
            self.amp_baseline.update(mean_amp);
            self.var_baseline.update(var);
            if let Some(h) = head_height_proxy { self.head_height_baseline.update(h); }
        }

        let now = Instant::now();
        let z_amp = self.amp_baseline.z(mean_amp);
        self.last_impact_z = z_var.max(z_amp.abs());

        // RuView reinforcement: head-height proxy drops sharply
        let ruview_drop = match (self.ruview_mode, head_height_proxy) {
            (true, Some(h)) if self.head_height_baseline.count > 10 => {
                let baseline_h = self.head_height_baseline.mean;
                if baseline_h > 1e-6 && h < baseline_h * 0.25 { true } else { false }
            }
            _ => false,
        };

        match self.state {
            State::Cooldown { until } if now < until => Report::cooldown(self.last_impact_z, self.total_falls),
            State::Cooldown { .. } => { self.state = State::Quiet; Report::quiet(0.0, self.total_falls) }
            State::Quiet => {
                let impact = z_var > self.impact_threshold && z_amp.abs() > self.impact_threshold * 0.5;
                if impact || ruview_drop {
                    self.state = State::Monitoring {
                        until: now + Duration::from_secs(self.stillness_window_secs),
                        stillness_frames: 0,
                    };
                    Report::impact(self.last_impact_z, ruview_drop, self.total_falls)
                } else {
                    Report::quiet(self.last_impact_z, self.total_falls)
                }
            }
            State::Monitoring { until, mut stillness_frames } => {
                if now > until {
                    self.state = State::Quiet;
                    return Report::quiet(self.last_impact_z, self.total_falls);
                }
                // Stillness = much lower variance than baseline
                let still = var < self.var_baseline.mean.max(1e-9) * 0.3;
                if still { stillness_frames += 1; } else { stillness_frames = 0; }

                if stillness_frames >= 5 {
                    self.total_falls += 1;
                    self.state = State::Cooldown { until: now + Duration::from_secs(self.cooldown_secs) };
                    let mut conf = (self.last_impact_z / 10.0).clamp(0.0, 1.0);
                    if ruview_drop { conf = (conf + 0.2).min(1.0); }
                    return Report::fall(conf, self.last_impact_z, self.total_falls);
                }

                self.state = State::Monitoring { until, stillness_frames };
                Report::monitoring(self.last_impact_z, stillness_frames, self.total_falls)
            }
        }
    }
}

#[derive(serde::Serialize)]
struct Report {
    status: String,
    fall_detected: bool,
    confidence: f64,
    z_impact: f64,
    stillness_pct: f64,
    total_falls: u64,
    timestamp: u64,
}

impl Report {
    fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
    fn quiet(z: f64, total: u64) -> Self { Self { status: "quiet".into(), fall_detected: false, confidence: 0.0, z_impact: z, stillness_pct: 0.0, total_falls: total, timestamp: Self::now_ts() } }
    fn impact(z: f64, ruview: bool, total: u64) -> Self { Self { status: if ruview { "impact+ruview".into() } else { "impact".into() }, fall_detected: false, confidence: (z / 10.0).clamp(0.0, 1.0), z_impact: z, stillness_pct: 0.0, total_falls: total, timestamp: Self::now_ts() } }
    fn monitoring(z: f64, frames: u32, total: u64) -> Self { Self { status: "monitoring".into(), fall_detected: false, confidence: 0.0, z_impact: z, stillness_pct: (frames as f64 / 5.0).min(1.0), total_falls: total, timestamp: Self::now_ts() } }
    fn fall(conf: f64, z: f64, total: u64) -> Self { Self { status: "FALL_DETECTED".into(), fall_detected: true, confidence: conf, z_impact: z, stillness_pct: 1.0, total_falls: total, timestamp: Self::now_ts() } }
    fn cooldown(z: f64, total: u64) -> Self { Self { status: "cooldown".into(), fall_detected: false, confidence: 0.0, z_impact: z, stillness_pct: 0.0, total_falls: total, timestamp: Self::now_ts() } }
}

fn fetch_sensors() -> Result<serde_json::Value, String> { cog_sensor_sources::fetch_sensors() }

fn store_to_seed(report: &Report) -> Result<(), String> {
    let v = vec![
        if report.fall_detected { 1.0 } else { 0.0 },
        report.confidence,
        (report.z_impact.abs() / 10.0).min(1.0),
        report.stillness_pct,
        report.total_falls as f64 / 100.0,
        0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[2, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(1);
    let impact_threshold: f64 = parse_arg(&args, "--impact-threshold").unwrap_or(6.0);
    let stillness_window: u64 = parse_arg(&args, "--stillness-window").unwrap_or(8);
    let cooldown: u64 = parse_arg(&args, "--cooldown").unwrap_or(30);
    let ruview_mode = args.iter().any(|a| a == "--ruview-mode");

    eprintln!("[cog-fall-detect] start (interval={interval}s, impact_threshold={impact_threshold}, stillness_window={stillness_window}s, cooldown={cooldown}s, ruview={ruview_mode})");

    let mut det = FallDetector::new(impact_threshold, stillness_window, cooldown, ruview_mode);

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(s) => {
                let samples = s.get("samples").and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    let amps: Vec<f64> = chs.iter().take(256).filter_map(|ch| ch.get("value").and_then(|v| v.as_f64())).collect();
                    if !amps.is_empty() {
                        let mean = amps.iter().sum::<f64>() / amps.len() as f64;
                        let var = amps.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / amps.len() as f64;
                        // Head-height proxy: use channel 0 absolute value if ruview mode
                        let head_h = if ruview_mode { amps.first().map(|v| v.abs()) } else { None };
                        let report = det.update(mean, var, head_h);
                        println!("{}", serde_json::to_string(&report).unwrap_or_default());
                        if let Err(e) = store_to_seed(&report) { eprintln!("[cog-fall-detect] store error: {e}"); }
                        if report.fall_detected {
                            eprintln!("[cog-fall-detect] ALERT: fall detected (confidence={:.0}%)", report.confidence * 100.0);
                        }
                    }
                }
            }
            Err(e) => eprintln!("[cog-fall-detect] sensor error: {e}"),
        }
        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
