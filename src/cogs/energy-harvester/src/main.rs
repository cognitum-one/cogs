//! Cognitum Cog: Energy Harvester Monitor
//!
//! Monitors power metrics (voltage/current sensors). Tracks solar input
//! vs battery drain. Computes energy balance and recommends duty cycle
//! for off-grid deployments.
//!
//! Expects sensor channels:
//!   ch0 = solar voltage (V), ch1 = solar current (A)
//!   ch2 = battery voltage (V), ch3 = battery current (A, negative=drain)
//!   ch4..ch7 = auxiliary
//!
//! Usage:
//!   cog-energy-harvester --once
//!   cog-energy-harvester --interval 10

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const BATTERY_LOW_V: f64 = 3.3;
const BATTERY_CRITICAL_V: f64 = 3.0;
const BATTERY_FULL_V: f64 = 4.2;

struct EnergyState {
    cumulative_solar_wh: f64,
    cumulative_drain_wh: f64,
    sample_count: u64,
    last_battery_v: f64,
    duty_cycle_pct: f64,
}

impl EnergyState {
    fn new() -> Self {
        Self {
            cumulative_solar_wh: 0.0,
            cumulative_drain_wh: 0.0,
            sample_count: 0,
            last_battery_v: 0.0,
            duty_cycle_pct: 100.0,
        }
    }
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
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
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store_vector(vec: &[f64; 8]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, vec.to_vec()]], "dedup": true });
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

#[derive(serde::Serialize)]
struct EnergyReport {
    solar_voltage: f64,
    solar_current: f64,
    solar_power_w: f64,
    battery_voltage: f64,
    battery_current: f64,
    battery_power_w: f64,
    energy_balance_w: f64,
    battery_pct: f64,
    battery_status: String,
    cumulative_solar_wh: f64,
    cumulative_drain_wh: f64,
    recommended_duty_cycle_pct: f64,
    alert: bool,
    alert_reason: Option<String>,
    timestamp: u64,
}

fn battery_pct(voltage: f64) -> f64 {
    // Simple linear approximation for LiPo
    let pct = (voltage - BATTERY_CRITICAL_V) / (BATTERY_FULL_V - BATTERY_CRITICAL_V) * 100.0;
    pct.max(0.0).min(100.0)
}

fn recommend_duty_cycle(battery_v: f64, solar_power: f64, drain_power: f64) -> f64 {
    if battery_v <= BATTERY_CRITICAL_V {
        10.0 // Emergency mode
    } else if battery_v <= BATTERY_LOW_V {
        25.0 // Power saving
    } else if solar_power > drain_power * 1.5 {
        100.0 // Full duty — surplus solar
    } else if solar_power > drain_power {
        80.0  // Slight conservation
    } else if solar_power > drain_power * 0.5 {
        50.0  // Moderate conservation
    } else {
        30.0  // Heavy conservation
    }
}

fn run_once(state: &mut EnergyState, interval_secs: f64) -> Result<EnergyReport, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples")
        .and_then(|c| c.as_array())
        .ok_or("no samples")?;

    let get_ch = |idx: usize| -> f64 {
        samples.get(idx)
            .and_then(|s| s.get("value").and_then(|v| v.as_f64()))
            .unwrap_or(0.0)
    };

    let solar_v = get_ch(0).abs();
    let solar_i = get_ch(1).abs();
    let battery_v = get_ch(2).abs();
    let battery_i = get_ch(3);  // negative = discharging

    let solar_power = solar_v * solar_i;
    let battery_power = battery_v * battery_i.abs();
    let balance = solar_power - battery_power;

    // Accumulate energy (Wh)
    let hours = interval_secs / 3600.0;
    state.cumulative_solar_wh += solar_power * hours;
    state.cumulative_drain_wh += battery_power * hours;
    state.sample_count += 1;
    state.last_battery_v = battery_v;

    let duty = recommend_duty_cycle(battery_v, solar_power, battery_power);
    state.duty_cycle_pct = duty;

    let bat_pct = battery_pct(battery_v);
    let status = if battery_v <= BATTERY_CRITICAL_V { "critical" }
    else if battery_v <= BATTERY_LOW_V { "low" }
    else if bat_pct > 90.0 { "full" }
    else { "normal" };

    let alert = battery_v <= BATTERY_LOW_V;
    let alert_reason = if battery_v <= BATTERY_CRITICAL_V {
        Some(format!("CRITICAL: Battery at {battery_v:.2}V — shutting down non-essential"))
    } else if battery_v <= BATTERY_LOW_V {
        Some(format!("Battery low: {battery_v:.2}V — duty cycle reduced to {duty:.0}%"))
    } else { None };

    let vector = [
        solar_power / 10.0,
        battery_power / 10.0,
        balance / 10.0,
        bat_pct / 100.0,
        duty / 100.0,
        solar_v / 20.0,
        battery_v / 5.0,
        if alert { 1.0 } else { 0.0 },
    ];
    let _ = store_vector(&vector);

    Ok(EnergyReport {
        solar_voltage: solar_v,
        solar_current: solar_i,
        solar_power_w: solar_power,
        battery_voltage: battery_v,
        battery_current: battery_i,
        battery_power_w: battery_power,
        energy_balance_w: balance,
        battery_pct: bat_pct,
        battery_status: status.to_string(),
        cumulative_solar_wh: state.cumulative_solar_wh,
        cumulative_drain_wh: state.cumulative_drain_wh,
        recommended_duty_cycle_pct: duty,
        alert,
        alert_reason,
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
        .unwrap_or(10);

    eprintln!("[cog-energy-harvester] starting (interval={interval}s)");
    let mut state = EnergyState::new();

    loop {
        let start = Instant::now();
        match run_once(&mut state, interval as f64) {
            Ok(report) => {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
                if report.alert {
                    eprintln!("[cog-energy-harvester] ALERT: {}", report.alert_reason.as_deref().unwrap_or("unknown"));
                }
            }
            Err(e) => eprintln!("[cog-energy-harvester] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
