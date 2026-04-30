//! Cognitum Cog: PPE Compliance (ADR-009)
//!
//! Cog-composition layer: fuses ruview-densepose presence detection
//! with PPE-camera-cog confirmation vectors. Fires when presence is
//! observed in a restricted zone without a confirming PPE check.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

#[derive(serde::Serialize)]
struct Report {
    status: String,
    violations_session: u64,
    presence_in_zone: bool,
    ppe_confirmed: bool,
    since_confirmation_secs: u64,
    timestamp: u64,
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

/// Query the seed RuVector store for the most recent vector with the
/// given prefix-byte tag.
fn search_recent(tag_byte: u8, limit: u32) -> Result<serde_json::Value, String> {
    let payload = serde_json::json!({ "tag": tag_byte, "limit": limit });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let target = std::env::var("COG_SENSOR_URL").unwrap_or_else(|_| "127.0.0.1:80".to_string());
    let host = target.split(':').next().unwrap_or("127.0.0.1").to_string();
    let mut c = std::net::TcpStream::connect(&target).map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/search HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new();
    c.read_to_end(&mut resp).ok();
    let body_text = String::from_utf8_lossy(&resp);
    let json_start = body_text.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body_text[json_start..]).map_err(|e| format!("parse: {e}"))
}

fn store(r: &Report) -> Result<(), String> {
    let v = vec![
        if r.presence_in_zone { 1.0 } else { 0.0 },
        if r.ppe_confirmed { 1.0 } else { 0.0 },
        (r.since_confirmation_secs as f64 / 600.0).min(1.0),
        (r.violations_session as f64 / 100.0).min(1.0),
        0.0, 0.0, 0.0, 0.0,
    ];
    let payload = serde_json::json!({ "vectors": [[9, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let target = std::env::var("COG_SENSOR_URL").unwrap_or_else(|_| "127.0.0.1:80".to_string());
    let mut c = std::net::TcpStream::connect(&target).map_err(|e| format!("connect: {e}"))?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    c.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(c, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| format!("w: {e}"))?;
    c.write_all(&body).map_err(|e| format!("body: {e}"))?;
    let mut resp = Vec::new(); c.read_to_end(&mut resp).ok();
    Ok(())
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<T>().ok())
}

fn parse_str_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval: u64 = parse_arg(&args, "--interval").unwrap_or(5);
    let _zone = parse_str_arg(&args, "--zone").unwrap_or_else(|| "restricted".to_string());
    let confirmation_window: u64 = parse_arg(&args, "--confirmation-window").unwrap_or(60);

    eprintln!("[cog-ppe-compliance] start (interval={interval}s, window={confirmation_window}s)");

    let mut violations: u64 = 0;
    // The skeleton tag from ruview-densepose is byte 0 in the existing cog
    const SKELETON_TAG: u8 = 0;
    // PPE camera cogs are conventionally tagged with byte 90+ ("vendor reserved")
    const PPE_TAG: u8 = 90;

    let mut last_presence: Option<Instant> = None;
    let mut last_ppe: Option<Instant> = None;

    loop {
        let start = Instant::now();
        let now = Instant::now();

        // Read latest skeleton vector — proxy for "presence in scene"
        let presence = matches!(search_recent(SKELETON_TAG, 1), Ok(j) if j.get("results").and_then(|r| r.as_array()).map(|a| !a.is_empty()).unwrap_or(false));
        if presence { last_presence = Some(now); }

        // Read latest PPE confirmation vector
        let ppe = matches!(search_recent(PPE_TAG, 1), Ok(j) if j.get("results").and_then(|r| r.as_array()).map(|a| !a.is_empty()).unwrap_or(false));
        if ppe { last_ppe = Some(now); }

        let since_ppe = last_ppe.map(|t| now.duration_since(t).as_secs()).unwrap_or(u64::MAX);
        let confirmation_fresh = since_ppe < confirmation_window;

        let mut violation = false;
        let status = match (presence, confirmation_fresh) {
            (true, true) => "compliant",
            (true, false) => {
                violation = true;
                violations += 1;
                "NON_COMPLIANT"
            }
            (false, _) if last_presence.is_some() && last_presence.unwrap().elapsed() < Duration::from_secs(confirmation_window * 2) => "warning",
            (false, _) => "compliant",
        };

        let r = Report {
            status: status.into(),
            violations_session: violations,
            presence_in_zone: presence,
            ppe_confirmed: confirmation_fresh,
            since_confirmation_secs: since_ppe.min(u64::MAX / 2),
            timestamp: now_ts(),
        };
        println!("{}", serde_json::to_string(&r).unwrap_or_default());
        if let Err(e) = store(&r) { eprintln!("[cog-ppe-compliance] store error: {e}"); }
        if violation { eprintln!("[cog-ppe-compliance] VIOLATION: presence in zone without PPE confirmation"); }

        if once { break; }
        let el = start.elapsed();
        if el < Duration::from_secs(interval) { std::thread::sleep(Duration::from_secs(interval) - el); }
    }
}
