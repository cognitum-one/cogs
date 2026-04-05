//! Cognitum Cog: Air Quality Index
//!
//! Reads environmental sensor channels (PM2.5, CO2 proxies). Computes AQI
//! from PM2.5 using EPA breakpoint formulas. Tracks trends with Welford
//! online statistics. Channels are mapped by name or index to pollutant
//! types.
//!
//! EPA AQI Breakpoints for PM2.5 (24-hr, ug/m3):
//!   Good:         0.0-12.0    -> AQI 0-50
//!   Moderate:     12.1-35.4   -> AQI 51-100
//!   Unhealthy-SG: 35.5-55.4   -> AQI 101-150
//!   Unhealthy:    55.5-150.4  -> AQI 151-200
//!   Very Unhealthy: 150.5-250.4 -> AQI 201-300
//!   Hazardous:    250.5-500.4 -> AQI 301-500
//!
//! Usage:
//!   cog-air-quality-index --once
//!   cog-air-quality-index --interval 30

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
    fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }
    fn std_dev(&self) -> f64 { self.variance().sqrt() }
}

/// EPA PM2.5 breakpoint table: (C_low, C_high, AQI_low, AQI_high)
const PM25_BREAKPOINTS: [(f64, f64, f64, f64); 6] = [
    (0.0,   12.0,  0.0,   50.0),
    (12.1,  35.4,  51.0,  100.0),
    (35.5,  55.4,  101.0, 150.0),
    (55.5,  150.4, 151.0, 200.0),
    (150.5, 250.4, 201.0, 300.0),
    (250.5, 500.4, 301.0, 500.0),
];

/// Compute AQI from PM2.5 concentration using EPA linear interpolation
fn pm25_to_aqi(pm25: f64) -> f64 {
    if pm25 < 0.0 { return 0.0; }
    if pm25 > 500.4 { return 500.0; }

    for &(c_low, c_high, aqi_low, aqi_high) in &PM25_BREAKPOINTS {
        if pm25 >= c_low && pm25 <= c_high {
            return aqi_low + (aqi_high - aqi_low) / (c_high - c_low) * (pm25 - c_low);
        }
    }
    500.0
}

/// CO2 to approximate indoor AQI category
/// (Not EPA standard, but useful proxy for indoor air quality)
fn co2_category(co2_ppm: f64) -> (String, f64) {
    if co2_ppm < 400.0 {
        ("excellent".into(), 0.0)
    } else if co2_ppm < 1000.0 {
        ("good".into(), (co2_ppm - 400.0) / 600.0 * 50.0)
    } else if co2_ppm < 2000.0 {
        ("moderate".into(), 50.0 + (co2_ppm - 1000.0) / 1000.0 * 50.0)
    } else if co2_ppm < 5000.0 {
        ("poor".into(), 100.0 + (co2_ppm - 2000.0) / 3000.0 * 100.0)
    } else {
        ("dangerous".into(), 200.0)
    }
}

fn aqi_category(aqi: f64) -> &'static str {
    if aqi <= 50.0 { "Good" }
    else if aqi <= 100.0 { "Moderate" }
    else if aqi <= 150.0 { "Unhealthy for Sensitive Groups" }
    else if aqi <= 200.0 { "Unhealthy" }
    else if aqi <= 300.0 { "Very Unhealthy" }
    else { "Hazardous" }
}

#[derive(serde::Serialize)]
struct AqiReport {
    pm25_proxy: f64,
    co2_proxy: f64,
    aqi: f64,
    aqi_category: String,
    co2_category: String,
    co2_index: f64,
    trend_direction: String,
    trend_mean: f64,
    trend_std_dev: f64,
    alerts: Vec<String>,
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

fn store_report(report: &AqiReport) -> Result<(), String> {
    let cat_code = match report.aqi_category.as_str() {
        "Good" => 0.0, "Moderate" => 0.2,
        "Unhealthy for Sensitive Groups" => 0.4,
        "Unhealthy" => 0.6, "Very Unhealthy" => 0.8,
        "Hazardous" => 1.0, _ => 0.1,
    };
    let vector = vec![
        report.aqi / 500.0,
        report.pm25_proxy / 500.0,
        report.co2_proxy / 5000.0,
        report.co2_index / 200.0,
        cat_code,
        report.trend_mean / 500.0,
        report.trend_std_dev / 100.0,
        if report.alerts.is_empty() { 0.0 } else { 1.0 },
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
        .unwrap_or(30);

    eprintln!("[cog-air-quality-index] starting (interval={}s)", interval);

    let mut aqi_trend = WelfordStats::new();
    let mut prev_aqi: Option<f64> = None;

    loop {
        let start = Instant::now();
        match fetch_sensors() {
            Ok(sensors) => {
                let samples = sensors.get("samples").and_then(|c| c.as_array());
                if let Some(chs) = samples {
                    // Map channels to pollutants by name or index
                    let mut pm25_vals = Vec::new();
                    let mut co2_vals = Vec::new();
                    let mut other_vals = Vec::new();

                    for ch in chs {
                        let channel = ch.get("channel").and_then(|c| c.as_str()).unwrap_or("");
                        let val = ch.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let ch_lower = channel.to_lowercase();
                        if ch_lower.contains("pm25") || ch_lower.contains("pm2.5") || ch_lower.contains("dust") {
                            pm25_vals.push(val);
                        } else if ch_lower.contains("co2") || ch_lower.contains("carbon") {
                            co2_vals.push(val);
                        } else {
                            other_vals.push(val);
                        }
                    }

                    // Use proxy values from available channels
                    // If no specific env channels, use signal magnitude as synthetic proxy
                    let pm25 = if !pm25_vals.is_empty() {
                        pm25_vals.iter().sum::<f64>() / pm25_vals.len() as f64
                    } else if !other_vals.is_empty() {
                        // Synthetic: scale signal variance to PM2.5-like range (0-100)
                        let mean = other_vals.iter().sum::<f64>() / other_vals.len() as f64;
                        let var = other_vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / other_vals.len().max(1) as f64;
                        (var.sqrt() * 10.0).min(500.0)
                    } else {
                        0.0
                    };

                    let co2 = if !co2_vals.is_empty() {
                        co2_vals.iter().sum::<f64>() / co2_vals.len() as f64
                    } else if other_vals.len() >= 2 {
                        // Synthetic: use second channel mean scaled to CO2 range
                        let second_half = &other_vals[other_vals.len() / 2..];
                        let mean = second_half.iter().sum::<f64>() / second_half.len() as f64;
                        (mean.abs() * 100.0 + 400.0).min(5000.0)
                    } else {
                        400.0
                    };

                    let aqi = pm25_to_aqi(pm25);
                    let category = aqi_category(aqi).to_string();
                    let (co2_cat, co2_idx) = co2_category(co2);

                    aqi_trend.update(aqi);
                    let trend = match prev_aqi {
                        Some(prev) if aqi > prev + 5.0 => "worsening",
                        Some(prev) if aqi < prev - 5.0 => "improving",
                        _ => "stable",
                    };
                    prev_aqi = Some(aqi);

                    let mut alerts = Vec::new();
                    if aqi > 150.0 {
                        alerts.push(format!("UNHEALTHY_AIR: AQI={:.0} ({})", aqi, category));
                    }
                    if co2 > 2000.0 {
                        alerts.push(format!("HIGH_CO2: {:.0} ppm ({})", co2, co2_cat));
                    }
                    if trend == "worsening" && aqi > 100.0 {
                        alerts.push(format!("AQI_WORSENING: trend from {:.0} to {:.0}", prev_aqi.unwrap_or(0.0), aqi));
                    }

                    let report = AqiReport {
                        pm25_proxy: pm25,
                        co2_proxy: co2,
                        aqi,
                        aqi_category: category,
                        co2_category: co2_cat,
                        co2_index: co2_idx,
                        trend_direction: trend.into(),
                        trend_mean: aqi_trend.mean,
                        trend_std_dev: aqi_trend.std_dev(),
                        alerts: alerts.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    println!("{}", serde_json::to_string(&report).unwrap_or_default());
                    if let Err(e) = store_report(&report) {
                        eprintln!("[cog-air-quality-index] store error: {e}");
                    }
                    if !alerts.is_empty() {
                        eprintln!("[cog-air-quality-index] ALERT: {:?}", alerts);
                    }
                }
            }
            Err(e) => eprintln!("[cog-air-quality-index] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
