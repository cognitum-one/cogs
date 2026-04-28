//! ADR-091: shared self-contained sensor sources for cogs.
//!
//! Provides one function: [`fetch_sensors`] which returns the same JSON
//! shape every cog's existing `fetch_sensors()` did (a `serde_json::Value`
//! with `"samples": [{"value": f}, ...]`), but with a smarter source-
//! selection strategy:
//!
//!   1. Try a UDP probe on `0.0.0.0:5006` (ADR-069 MAGIC_FEATURES) for
//!      2s. If any packets arrive, decode their 8 LE-f32 features into
//!      a synthetic samples array tagged `sensor: "esp32-udp"`.
//!   2. Otherwise fall back to `GET 127.0.0.1:80/api/v1/sensor/stream`.
//!
//! The probe runs once per call. For cogs that read at 1 Hz this is
//! fine; cogs that need lower latency should latch the source at
//! startup with [`probe_source`] + [`fetch_from_seed_stream`] /
//! [`fetch_from_udp_window`] directly.
//!
//! No `--source` flag here. Cogs that want explicit control still
//! parse `--source` themselves and call [`fetch_from_seed_stream`]
//! / [`fetch_from_udp_window`] directly.

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC_FEATURES: u32 = 0xC511_0003;
const FEATURE_PKT_SIZE: usize = 48;
const DEFAULT_UDP_BIND: &str = "0.0.0.0:5006";
const DEFAULT_PROBE_MS: u64 = 2000;

/// Drop-in replacement for the per-cog `fetch_sensors()` function.
/// Tries ESP32 UDP first, falls back to seed-stream. Returns the same
/// `{"samples": [{"value": f, ...}, ...]}` shape so cogs need no other
/// changes.
pub fn fetch_sensors() -> Result<serde_json::Value, String> {
    match fetch_from_udp_window(DEFAULT_UDP_BIND, DEFAULT_PROBE_MS) {
        Ok(values) if !values.is_empty() => Ok(udp_values_to_json(&values, "esp32-udp")),
        _ => fetch_from_seed_stream(),
    }
}

/// Bind UDP, accept ADR-069 MAGIC_FEATURES packets up to `window_ms`,
/// return the decoded f64 features.
pub fn fetch_from_udp_window(bind: &str, window_ms: u64) -> Result<Vec<f64>, String> {
    let socket = UdpSocket::bind(bind).map_err(|e| format!("bind {}: {}", bind, e))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(window_ms.min(2000))))
        .map_err(|e| format!("set timeout: {}", e))?;

    let mut amps: Vec<f64> = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(window_ms);
    let mut pkt = [0u8; 256];
    while Instant::now() < deadline && amps.len() < 256 {
        match socket.recv_from(&mut pkt) {
            Ok((n, _)) if n >= FEATURE_PKT_SIZE => {
                let magic = u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]);
                if magic != MAGIC_FEATURES {
                    continue;
                }
                for i in 0..8 {
                    let off = 16 + i * 4;
                    if off + 4 > n {
                        break;
                    }
                    let f = f32::from_le_bytes([pkt[off], pkt[off + 1], pkt[off + 2], pkt[off + 3]]);
                    if f.is_finite() {
                        amps.push((f as f64).clamp(-1.0, 1.0));
                    }
                }
            }
            Ok(_) => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("udp recv: {}", e)),
        }
    }
    if amps.is_empty() {
        Err(format!("no ADR-069 packets on {} within {}ms", bind, window_ms))
    } else {
        Ok(amps)
    }
}

/// Original behavior: HTTP GET against the agent's loopback sensor stream.
pub fn fetch_from_seed_stream() -> Result<serde_json::Value, String> {
    let mut conn = TcpStream::connect("127.0.0.1:80").map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(
        conn,
        "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match conn.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() > 262144 {
                    break;
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break
            }
            Err(_) if !buf.is_empty() => break,
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    let body = String::from_utf8_lossy(&buf);
    let json_start = body.find('{').ok_or("no JSON")?;
    serde_json::from_str(&body[json_start..]).map_err(|e| format!("parse: {e}"))
}

/// Dump UDP-derived features into the same `{"samples": [...]}` shape
/// the agent's `/api/v1/sensor/stream` returns, so existing cog DSP
/// code can consume it unchanged.
fn udp_values_to_json(values: &[f64], sensor_tag: &str) -> serde_json::Value {
    let samples: Vec<serde_json::Value> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            serde_json::json!({
                "channel": format!("ch{}", i),
                "value": v,
                "normalized": v,
                "quality": 0,
                "quality_label": "good",
                "sensor": sensor_tag,
                "timestamp_us": 0u64,
            })
        })
        .collect();
    serde_json::json!({
        "healthy": true,
        "sample_count": samples.len(),
        "sample_rate_hz": 10,
        "total_channels": samples.len(),
        "samples": samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_values_to_json_matches_sensor_stream_shape() {
        let v = udp_values_to_json(&[0.1, 0.2, 0.3], "test");
        assert!(v.get("samples").is_some());
        let samples = v["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0]["value"], 0.1);
        assert_eq!(samples[0]["sensor"], "test");
    }

    #[test]
    fn udp_window_rejects_non_magic_packets() {
        // bind a port; send a junk packet from another socket; expect Err
        use std::thread;
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let bind = format!("127.0.0.1:{}", port);
        thread::spawn({
            let bind2 = bind.clone();
            move || {
                let s = UdpSocket::bind("127.0.0.1:0").unwrap();
                let mut buf = [0u8; 64];
                buf[..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
                let _ = s.send_to(&buf, &bind2);
            }
        });
        drop(listener);
        thread::sleep(Duration::from_millis(50));
        let result = fetch_from_udp_window(&bind, 500);
        assert!(result.is_err() || result.unwrap().is_empty());
    }
}
