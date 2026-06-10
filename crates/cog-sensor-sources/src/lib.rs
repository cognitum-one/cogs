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

/// ADR-069 / ADR-063 device-computed vitals packet (`edge_vitals_pkt_t`,
/// 32 bytes, `__attribute__((packed))`, little-endian). The ESP32 already runs
/// the breathing/heart-rate/presence estimation on-device (the estimate fixed
/// and Apple-Watch-validated in firmware v0.7.1), so cogs should prefer this
/// over re-deriving vitals from the raw feature packet.
const MAGIC_VITALS: u32 = 0xC511_0002;

/// Device-computed vitals decoded from a `MAGIC_VITALS` (0xC5110002) packet.
#[derive(Debug, Clone, Default)]
pub struct Esp32Vitals {
    /// `flags` bit0 — a person is present per the device's own detector.
    pub presence: bool,
    /// Heart rate in BPM (`heartrate` field / 10000).
    pub heart_rate_bpm: f64,
    /// Breathing rate in BPM (`breathing_rate` field / 100).
    pub breathing_bpm: f64,
    /// Number of persons the device is tracking.
    pub n_persons: u8,
    /// Continuous presence score (higher = stronger presence evidence).
    pub presence_score: f32,
    /// Phase-variance / motion metric.
    pub motion_energy: f32,
}

/// One cycle of drained ESP32 data: raw `features` (0xC5110003) for cogs that
/// still want to run their own DSP, plus the latest device-computed `vitals`
/// (0xC5110002) if any arrived this cycle.
#[derive(Debug, Clone, Default)]
pub struct Esp32Frame {
    pub features: Vec<f64>,
    pub vitals: Option<Esp32Vitals>,
}

/// Decode a `MAGIC_VITALS` packet from `pkt` (length `pkt.len()` = bytes
/// received). Returns `None` if the buffer is too short for the fields required
/// to be meaningful. This is an unauthenticated network boundary (`0.0.0.0:5006`)
/// — every field is bounds-checked against the received length, never via a
/// caller "guarantee".
fn decode_vitals(pkt: &[u8]) -> Option<Esp32Vitals> {
    // 0:magic u32  4:node_id u8  5:flags u8  6:breathing_rate u16 (bpm*100)
    // 8:heartrate u32 (bpm*10000)  12:rssi i8  13:n_persons u8  14:reserved[2]
    // 16:motion_energy f32  20:presence_score f32  24:timestamp_ms u32  (32 bytes)
    let flags = *pkt.get(5)?;
    let breathing_bpm = u16::from_le_bytes([*pkt.get(6)?, *pkt.get(7)?]) as f64 / 100.0;
    let heart_rate_bpm =
        u32::from_le_bytes([*pkt.get(8)?, *pkt.get(9)?, *pkt.get(10)?, *pkt.get(11)?]) as f64
            / 10000.0;
    let n_persons = *pkt.get(13)?;
    // Optional trailing fields — present in the full 32-byte packet, defaulted
    // to 0.0 if a shorter (but still valid) packet omits them.
    let motion_energy = match (pkt.get(16), pkt.get(19)) {
        (Some(_), Some(_)) => f32::from_le_bytes([pkt[16], pkt[17], pkt[18], pkt[19]]),
        _ => 0.0,
    };
    let presence_score = match (pkt.get(20), pkt.get(23)) {
        (Some(_), Some(_)) => f32::from_le_bytes([pkt[20], pkt[21], pkt[22], pkt[23]]),
        _ => 0.0,
    };
    Some(Esp32Vitals {
        presence: flags & 0x01 != 0,
        heart_rate_bpm,
        breathing_bpm,
        n_persons,
        presence_score,
        motion_energy,
    })
}

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

/// A persistently-bound UDP listener for ADR-069 `MAGIC_FEATURES` packets.
///
/// `fetch_from_udp_window` binds a fresh socket on every call and reads for a
/// fixed window, so any packet that arrives while the socket is closed (between
/// a cog's read cycles) is dropped by the OS. At a multi-second cog interval
/// that drops ~half the windows, which forces a synthetic fall-back and
/// interleaves fake frames into stateful DSP. Bind this once at startup and
/// `drain()` each cycle instead: the socket stays open, the kernel buffers
/// packets between cycles, and the cog gets a continuous real feed with no
/// synthetic interleave.
pub struct Esp32UdpListener {
    socket: UdpSocket,
}

impl Esp32UdpListener {
    /// Bind once and keep the socket open (non-blocking) for the cog's lifetime.
    pub fn bind(addr: &str) -> Result<Self, String> {
        let socket = UdpSocket::bind(addr).map_err(|e| format!("bind {}: {}", addr, e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("set_nonblocking: {}", e))?;
        Ok(Self { socket })
    }

    /// Drain everything currently buffered: decode `MAGIC_FEATURES` packets
    /// into up to `max` clamped f64 amplitudes, and keep the latest
    /// `MAGIC_VITALS` packet seen this cycle. Non-blocking — returns whatever is
    /// available right now (possibly empty) and never substitutes synthetic
    /// data; the caller decides what an empty drain means.
    pub fn drain(&self, max: usize) -> Esp32Frame {
        let mut features: Vec<f64> = Vec::new();
        let mut vitals: Option<Esp32Vitals> = None;
        let mut pkt = [0u8; 256];
        loop {
            match self.socket.recv_from(&mut pkt) {
                Ok((n, _)) if n >= 4 => {
                    let magic = u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]);
                    if magic == MAGIC_FEATURES && n >= FEATURE_PKT_SIZE {
                        if features.len() < max {
                            for i in 0..8 {
                                let off = 16 + i * 4;
                                if off + 4 > n {
                                    break;
                                }
                                let f = f32::from_le_bytes([
                                    pkt[off],
                                    pkt[off + 1],
                                    pkt[off + 2],
                                    pkt[off + 3],
                                ]);
                                if f.is_finite() {
                                    features.push((f as f64).clamp(-1.0, 1.0));
                                }
                            }
                        }
                    } else if magic == MAGIC_VITALS {
                        // Keep the freshest *valid* vitals packet from this drain;
                        // decode bounds-checks against the received length.
                        if let Some(v) = decode_vitals(&pkt[..n]) {
                            vitals = Some(v);
                        }
                    }
                }
                Ok(_) => continue,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(_) => break,
            }
        }
        Esp32Frame { features, vitals }
    }
}

/// Like [`fetch_sensors`] but backed by a process-wide **persistent** UDP
/// listener, so ESP32 packets aren't dropped between calls (the gap that made
/// the per-call probe intermittent and forced synthetic interleave into stateful
/// DSP). Returns the same `{"samples":[...]}` shape — a drop-in for cogs that run
/// their own DSP over the raw feature feed (e.g. cardiac-arrhythmia's R-R / HRV,
/// densepose's pose keypoints) and therefore can't use the device vitals packet.
/// Falls back to seed-stream only when the socket can't bind or no ESP32 packets
/// are currently buffered.
fn last_vitals_cell() -> &'static std::sync::Mutex<Option<Esp32Vitals>> {
    static CELL: std::sync::OnceLock<std::sync::Mutex<Option<Esp32Vitals>>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| std::sync::Mutex::new(None))
}

/// The device-computed vitals seen on the most recent
/// [`fetch_sensors_persistent`] drain (`None` if that cycle carried no vitals
/// packet). Lets a cog that runs its own waveform DSP additionally surface the
/// device's authoritative presence / heart-rate / breathing — e.g. override a
/// re-derived breathing *rate* with the device value while keeping waveform DSP
/// for effort / Cheyne-Stokes / apnea pattern detection — without binding a
/// second socket. Call it right after `fetch_sensors_persistent()` in the same
/// cycle so the value is fresh.
pub fn latest_vitals() -> Option<Esp32Vitals> {
    last_vitals_cell().lock().ok().and_then(|g| g.clone())
}

pub fn fetch_sensors_persistent() -> Result<serde_json::Value, String> {
    static LISTENER: std::sync::OnceLock<Option<Esp32UdpListener>> = std::sync::OnceLock::new();
    let listener = LISTENER.get_or_init(|| match Esp32UdpListener::bind(DEFAULT_UDP_BIND) {
        Ok(l) => Some(l),
        Err(e) => {
            // Loud, once: a busy 5006 means another CSI cog holds it and this cog
            // will silently serve synthetic — exactly the symptom we just fixed.
            eprintln!(
                "[cog-sensor-sources] WARNING: cannot bind {DEFAULT_UDP_BIND} ({e}); \
                 another CSI cog likely holds it — serving seed-stream (synthetic) data"
            );
            None
        }
    });
    if let Some(l) = listener {
        let frame = l.drain(256);
        // Record this cycle's vitals (or None) so latest_vitals() reflects the
        // current drain, never a stale packet.
        if let Ok(mut g) = last_vitals_cell().lock() {
            *g = frame.vitals.clone();
        }
        if !frame.features.is_empty() {
            return Ok(udp_values_to_json(&frame.features, "esp32-udp"));
        }
    }
    fetch_from_seed_stream()
}

/// Original behavior: HTTP GET against the agent's loopback sensor stream.
///
/// Honors the `COG_SENSOR_URL` env var for off-device validation. Defaults
/// to `127.0.0.1:80` (loopback) when unset, matching original behavior.
/// Format: `host:port` (no scheme, no path — path is always
/// `/api/v1/sensor/stream`).
pub fn fetch_from_seed_stream() -> Result<serde_json::Value, String> {
    let target = std::env::var("COG_SENSOR_URL").unwrap_or_else(|_| "127.0.0.1:80".to_string());
    let host = target.split(':').next().unwrap_or("127.0.0.1").to_string();
    let mut conn = TcpStream::connect(&target).map_err(|e| format!("connect {target}: {e}"))?;
    conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(Duration::from_secs(5))).ok();
    write!(
        conn,
        "GET /api/v1/sensor/stream HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n"
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

    /// Build a full 32-byte vitals packet (`0xC5110002`) with known fields.
    fn vitals_pkt(presence: bool, breathing_x100: u16, hr_x10000: u32, n_persons: u8,
                  presence_score: f32) -> [u8; 32] {
        let mut p = [0u8; 32];
        p[0..4].copy_from_slice(&MAGIC_VITALS.to_le_bytes());
        p[5] = if presence { 0x01 } else { 0x00 };
        p[6..8].copy_from_slice(&breathing_x100.to_le_bytes());
        p[8..12].copy_from_slice(&hr_x10000.to_le_bytes());
        p[13] = n_persons;
        p[20..24].copy_from_slice(&presence_score.to_le_bytes());
        p
    }

    #[test]
    fn decode_vitals_golden() {
        // breathing 15.00 bpm, HR 72.0000 bpm, presence on, 1 person, score 12.5.
        let pkt = vitals_pkt(true, 1500, 720000, 1, 12.5);
        let v = decode_vitals(&pkt).expect("valid packet decodes");
        assert!(v.presence);
        assert!((v.breathing_bpm - 15.0).abs() < 1e-6, "breathing={}", v.breathing_bpm);
        assert!((v.heart_rate_bpm - 72.0).abs() < 1e-6, "hr={}", v.heart_rate_bpm);
        assert_eq!(v.n_persons, 1);
        assert!((v.presence_score - 12.5).abs() < 1e-6);
    }

    #[test]
    fn decode_vitals_absent_zero_breathing() {
        // No person, breathing 0 — must decode cleanly (apnea logic lives in the cog).
        let pkt = vitals_pkt(false, 0, 0, 0, 0.0);
        let v = decode_vitals(&pkt).unwrap();
        assert!(!v.presence);
        assert_eq!(v.breathing_bpm, 0.0);
        assert_eq!(v.heart_rate_bpm, 0.0);
    }

    #[test]
    fn decode_vitals_short_packet_is_none() {
        // Too short to reach n_persons@13 — must not panic, returns None.
        let mut p = [0u8; 12];
        p[0..4].copy_from_slice(&MAGIC_VITALS.to_le_bytes());
        assert!(decode_vitals(&p).is_none());
        // Empty / tiny buffers too.
        assert!(decode_vitals(&[]).is_none());
        assert!(decode_vitals(&[0u8; 4]).is_none());
    }

    #[test]
    fn drain_routes_vitals_and_features() {
        let listener = Esp32UdpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.socket.local_addr().unwrap();
        let tx = UdpSocket::bind("127.0.0.1:0").unwrap();

        // One vitals packet (breathing 18.0, HR 65.0, presence on)...
        tx.send_to(&vitals_pkt(true, 1800, 650000, 2, 9.0), addr).unwrap();
        // ...and one feature packet (0xC5110003: 8 f32 at offset 16).
        let mut fp = [0u8; FEATURE_PKT_SIZE];
        fp[0..4].copy_from_slice(&MAGIC_FEATURES.to_le_bytes());
        for i in 0..8 {
            fp[16 + i * 4..16 + i * 4 + 4].copy_from_slice(&0.5f32.to_le_bytes());
        }
        tx.send_to(&fp, addr).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let frame = listener.drain(256);
        let v = frame.vitals.expect("vitals decoded");
        assert!(v.presence);
        assert!((v.breathing_bpm - 18.0).abs() < 1e-6);
        assert!((v.heart_rate_bpm - 65.0).abs() < 1e-6);
        assert_eq!(v.n_persons, 2);
        assert_eq!(frame.features.len(), 8, "8 features decoded");
        assert!((frame.features[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn drain_empty_when_nothing_buffered() {
        let listener = Esp32UdpListener::bind("127.0.0.1:0").unwrap();
        let frame = listener.drain(256);
        assert!(frame.features.is_empty());
        assert!(frame.vitals.is_none());
    }
}
