//! cog-presence-field — multi-node WiFi-CSI presence via field-model residual.
//!
//! ADR-151. The shipped edge presence (ESP32 `motion_energy = var(phase)` over a
//! single node) is ambient-noise-dominated and cannot detect a still person.
//! Device data shows the **field-model residual** does: learn the empty-room
//! baseline per node (mean + top-K environmental eigenmodes via SVD/Jacobi), then
//!
//!     residual = (obs - mean) - Vk·Vkᵀ·(obs - mean)
//!     presence = max_over_nodes( ‖residual‖² / empty_floor ) > THRESH
//!
//! maxed across nodes for spatial coverage. Verified: still person ≈10×, moving
//! ≈60× the empty baseline (single node ≈1× — undetectable).
//!
//! Usage:
//!   cog-presence-field --calibrate 120     # learn empty room (LEAVE the room), save, then run
//!   cog-presence-field                     # load saved baseline, run live presence
//!   cog-presence-field --bind 0.0.0.0:5006 --modes 8 --thresh 4.0 --hold 5 --interval 1
//!
//! Reads raw CSI (ADR-018 `0xC5110001`) on UDP, separated by node_id (byte[4]).

use std::collections::HashMap;
use std::io::Read;
use std::net::UdpSocket;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CSI_MAGIC: u32 = 0xC511_0001;
const VITALS_MAGIC: u32 = 0xC511_0002;
const FEATURE_MAGIC: u32 = 0xC511_0003;
const FUSED_MAGIC: u32 = 0xC511_0004;
const N_SUB: usize = 64; // subcarriers (ESP32-S3 single antenna)

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct NodeModel {
    mean: Vec<f64>,          // [N_SUB]
    modes: Vec<Vec<f64>>,    // K eigenvectors, each [N_SUB] (top environmental modes)
    floor: f64,              // mean empty-room residual energy (threshold anchor)
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Baseline {
    n_modes: usize,
    nodes: HashMap<u8, NodeModel>,
}

// ── Jacobi symmetric eigendecomposition (pure Rust, no deps) ─────────────
/// Returns (eigenvalues, eigenvectors-as-columns) for a symmetric n×n matrix,
/// sorted by eigenvalue descending. Cyclic Jacobi rotations.
fn jacobi_eigh(mut a: Vec<Vec<f64>>) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    let mut v = vec![vec![0.0; n]; n];
    for i in 0..n { v[i][i] = 1.0; }
    for _sweep in 0..100 {
        // off-diagonal magnitude
        let mut off = 0.0;
        for p in 0..n { for q in (p + 1)..n { off += a[p][q] * a[p][q]; } }
        if off < 1e-12 { break; }
        for p in 0..n {
            for q in (p + 1)..n {
                if a[p][q].abs() < 1e-15 { continue; }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                // rotate rows/cols p,q
                for k in 0..n {
                    let akp = a[k][p];
                    let akq = a[k][q];
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p][k];
                    let aqk = a[q][k];
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let vkp = v[k][p];
                    let vkq = v[k][q];
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut evals: Vec<(f64, usize)> = (0..n).map(|i| (a[i][i], i)).collect();
    evals.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    let values: Vec<f64> = evals.iter().map(|e| e.0).collect();
    let vectors: Vec<Vec<f64>> = evals
        .iter()
        .map(|&(_, idx)| (0..n).map(|r| v[r][idx]).collect())
        .collect(); // each = one eigenvector [n]
    (values, vectors)
}

/// residual energy of `x` after removing baseline mean + the top-K modes.
fn residual_energy(x: &[f64], m: &NodeModel) -> f64 {
    // c = x - mean
    let mut c = vec![0.0; N_SUB];
    for i in 0..N_SUB { c[i] = x.get(i).copied().unwrap_or(0.0) - m.mean[i]; }
    // subtract projection onto each mode: c -= (c·v) v
    for v in &m.modes {
        let dot: f64 = (0..N_SUB).map(|i| c[i] * v[i]).sum();
        for i in 0..N_SUB { c[i] -= dot * v[i]; }
    }
    c.iter().map(|r| r * r).sum()
}

fn compute_model(frames: &[Vec<f64>], k: usize) -> Option<NodeModel> {
    let t = frames.len();
    if t < 30 { return None; }
    let mut mean = vec![0.0; N_SUB];
    for f in frames { for i in 0..N_SUB { mean[i] += f.get(i).copied().unwrap_or(0.0); } }
    for m in &mut mean { *m /= t as f64; }
    // covariance N_SUB×N_SUB
    let mut cov = vec![vec![0.0; N_SUB]; N_SUB];
    for f in frames {
        let mut c = [0.0f64; N_SUB];
        for i in 0..N_SUB { c[i] = f.get(i).copied().unwrap_or(0.0) - mean[i]; }
        for i in 0..N_SUB {
            for j in i..N_SUB {
                let val = c[i] * c[j];
                cov[i][j] += val;
                if i != j { cov[j][i] += val; }
            }
        }
    }
    let scale = 1.0 / (t as f64 - 1.0);
    for row in &mut cov { for x in row { *x *= scale; } }
    let (_vals, vecs) = jacobi_eigh(cov);
    let modes: Vec<Vec<f64>> = vecs.into_iter().take(k).collect();
    let mut m = NodeModel { mean, modes, floor: 1.0 };
    // empty floor = mean in-sample residual energy
    let fl: f64 = frames.iter().map(|f| residual_energy(f, &m)).sum::<f64>() / t as f64;
    m.floor = fl.max(1e-6);
    Some(m)
}

// ── Breathing-band still-person detector (ADR-151) ───────────────────────
// A motionless body fades out of the *spatial* residual (its static signature
// is absorbed into the empty-room subspace), but its **respiration keeps
// periodically modulating the field**. We detect a person at rest by finding a
// stable periodicity in the per-node residual time-series within the human
// respiration band (RESP_LO..RESP_HI Hz = 6..30 breaths/min) with enough power
// over the band's noise floor. This is the liveness signal the motion residual
// cannot see — verified against Vitality Call field data where a physiological,
// temporally-stable breathing rate was present in 99.8% of readings while
// motion-residual presence reported empty.
const RESP_LO_HZ: f64 = 0.10; // 6 bpm
const RESP_HI_HZ: f64 = 0.50; // 30 bpm
const RESP_MIN_SAMPLES: usize = 24; // need a few breath cycles before trusting a peak

/// Goertzel-style single-frequency power of a zero-mean series sampled at `fs`.
fn freq_power(x: &[f64], f: f64, fs: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * f / fs;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (k, &v) in x.iter().enumerate() {
        let a = w * k as f64;
        re += v * a.cos();
        im += v * a.sin();
    }
    (re * re + im * im) / (x.len() as f64).max(1.0)
}

/// Scan the respiration band of a residual time-series. Returns
/// `(breathing_bpm, snr)` for the strongest in-band peak, where `snr` = peak
/// power / median band power (noise floor). The caller gates on `snr` + temporal
/// stability so house/AP/printer noise (broadband, no stable in-band peak) does
/// not read as a person. Returns `None` when there is no usable signal.
fn breathing_band(series: &[f64], fs: f64) -> Option<(f64, f64)> {
    let n = series.len();
    if n < RESP_MIN_SAMPLES || fs <= 0.0 || fs / 2.0 <= RESP_LO_HZ {
        return None; // too few samples or Nyquist below the band
    }
    let mean = series.iter().sum::<f64>() / n as f64;
    let x: Vec<f64> = series.iter().map(|v| v - mean).collect();
    let var = x.iter().map(|v| v * v).sum::<f64>() / n as f64;
    if var <= f64::EPSILON {
        return None; // flat line — no modulation
    }
    let hi = RESP_HI_HZ.min(fs / 2.0 - 1e-6);
    let step = 0.005f64; // ~0.3 bpm grid
    let mut powers: Vec<(f64, f64)> = Vec::new();
    let mut f = RESP_LO_HZ;
    while f <= hi {
        powers.push((f, freq_power(&x, f, fs)));
        f += step;
    }
    if powers.len() < 3 {
        return None;
    }
    let (mut peak_f, mut peak_p) = (0.0f64, 0.0f64);
    for &(ff, pp) in &powers {
        if pp > peak_p {
            peak_p = pp;
            peak_f = ff;
        }
    }
    let mut ps: Vec<f64> = powers.iter().map(|&(_, p)| p).collect();
    ps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = ps[ps.len() / 2].max(f64::EPSILON); // band noise floor
    Some((peak_f * 60.0, peak_p / med))
}

// ── CSI decode ───────────────────────────────────────────────────────────
fn decode_csi(d: &[u8]) -> Option<(u8, Vec<f64>)> {
    if d.len() < 20 { return None; }
    let magic = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
    if magic != CSI_MAGIC { return None; }
    let node_id = d[4];
    let iq = &d[20..];
    let n = (iq.len() / 2).min(N_SUB);
    let mut amp = vec![0.0; N_SUB];
    for k in 0..n {
        let i = iq[2 * k] as i8 as f64;
        let q = iq[2 * k + 1] as i8 as f64;
        amp[k] = (i * i + q * q).sqrt();
    }
    Some((node_id, amp))
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned()
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Optional: persist a presence reading to the seed store (best-effort).
/// MUST NOT block the presence loop — uses a short connect timeout because on the
/// seed an unanswered SYN to :80 otherwise hangs the loop ~15s (frames go stale).
fn store_presence(present: bool, score: f64) {
    let vector = vec![if present { 1.0 } else { 0.0 }, (score / 100.0).min(1.0), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let payload = serde_json::json!({ "vectors": [[0, vector]], "dedup": true });
    let body = match serde_json::to_vec(&payload) { Ok(b) => b, Err(_) => return };
    let addr: std::net::SocketAddr = match "127.0.0.1:80".parse() { Ok(a) => a, Err(_) => return };
    if let Ok(mut conn) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
        let _ = conn.set_write_timeout(Some(Duration::from_millis(500)));
        let _ = conn.set_read_timeout(Some(Duration::from_millis(500)));
        use std::io::Write;
        let _ = write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len());
        let _ = conn.write_all(&body);
        let mut sink = Vec::new();
        let _ = conn.read_to_end(&mut sink);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bind = arg(&args, "--bind").unwrap_or_else(|| "0.0.0.0:5006".to_string());
    let k = arg(&args, "--modes").and_then(|s| s.parse().ok()).unwrap_or(8usize);
    let thresh = arg(&args, "--thresh").and_then(|s| s.parse().ok()).unwrap_or(4.0f64);
    let hold = arg(&args, "--hold").and_then(|s| s.parse().ok()).unwrap_or(5u64);
    let interval = arg(&args, "--interval").and_then(|s| s.parse().ok()).unwrap_or(1u64);
    let window = arg(&args, "--window").and_then(|s| s.parse().ok()).unwrap_or(20usize);
    // Breathing-band still-person detector (ADR-151): residual history window in
    // seconds, peak-over-floor SNR required, and consecutive-eval stability count.
    let breath_secs = arg(&args, "--breath-secs").and_then(|s| s.parse().ok()).unwrap_or(45u64);
    let breath_snr = arg(&args, "--breath-snr").and_then(|s| s.parse().ok()).unwrap_or(4.0f64);
    let breath_stable = arg(&args, "--breath-stable").and_then(|s| s.parse().ok()).unwrap_or(3usize);
    let baseline_path = arg(&args, "--baseline")
        .unwrap_or_else(|| "/var/lib/cognitum/apps/presence-field/baseline.json".to_string());
    let calibrate: Option<u64> = arg(&args, "--calibrate").and_then(|s| s.parse().ok());
    // Broker fan-out: relay vitals/feature packets to a loopback port so vitals
    // cogs can still consume them while presence-field owns 5006. Presence is
    // published to a small JSON file that Health Monitor reads to gate vitals.
    let relay_addr = arg(&args, "--relay").unwrap_or_else(|| "127.0.0.1:5106".to_string());
    let presence_file = arg(&args, "--presence-file")
        .unwrap_or_else(|| "/tmp/cognitum-presence.json".to_string());

    let socket = match UdpSocket::bind(&bind) {
        Ok(s) => s,
        Err(e) => { eprintln!("[presence-field] FATAL: cannot bind {bind}: {e} (another CSI cog may hold it)"); std::process::exit(2); }
    };
    socket.set_read_timeout(Some(Duration::from_millis(500))).ok();
    eprintln!("[presence-field] starting bind={bind} modes={k} thresh={thresh}x hold={hold}s");

    // ── Calibration mode ──────────────────────────────────────────────────
    if let Some(secs) = calibrate {
        eprintln!("[presence-field] CALIBRATING {secs}s — LEAVE THE ROOM (learning empty baseline)…");
        let mut per: HashMap<u8, Vec<Vec<f64>>> = HashMap::new();
        let mut buf = [0u8; 4096];
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if let Ok((n, _)) = socket.recv_from(&mut buf) {
                if let Some((nid, amp)) = decode_csi(&buf[..n]) { per.entry(nid).or_default().push(amp); }
            }
        }
        let mut base = Baseline { n_modes: k, nodes: HashMap::new() };
        for (nid, frames) in &per {
            match compute_model(frames, k) {
                Some(m) => { eprintln!("[presence-field] node{nid}: {} frames, empty_floor={:.3}", frames.len(), m.floor); base.nodes.insert(*nid, m); }
                None => eprintln!("[presence-field] node{nid}: too few frames ({}), skipped", frames.len()),
            }
        }
        if base.nodes.is_empty() { eprintln!("[presence-field] calibration FAILED — no CSI received"); std::process::exit(3); }
        if let Some(parent) = std::path::Path::new(&baseline_path).parent() { let _ = std::fs::create_dir_all(parent); }
        match std::fs::write(&baseline_path, serde_json::to_vec_pretty(&base).unwrap_or_default()) {
            Ok(_) => eprintln!("[presence-field] baseline saved -> {baseline_path} ({} nodes)", base.nodes.len()),
            Err(e) => { eprintln!("[presence-field] cannot save baseline: {e}"); std::process::exit(4); }
        }
    }

    // ── Load baseline ─────────────────────────────────────────────────────
    let base: Baseline = match std::fs::read(&baseline_path) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => { eprintln!("[presence-field] no baseline at {baseline_path} — run with --calibrate <secs> first"); std::process::exit(5); }
    };
    if base.nodes.is_empty() { eprintln!("[presence-field] baseline has no nodes"); std::process::exit(5); }
    eprintln!("[presence-field] running: {} calibrated node(s)", base.nodes.len());

    // ── Runtime loop ──────────────────────────────────────────────────────
    let mut ring: HashMap<u8, std::collections::VecDeque<f64>> = HashMap::new();
    // Per-node residual history (Instant, energy) for the breathing-band detector;
    // kept for `breath_secs` so a respiration peak has several cycles to resolve.
    let mut hist: HashMap<u8, std::collections::VecDeque<(Instant, f64)>> = HashMap::new();
    // Recent in-band breathing-rate estimates, for temporal-stability gating.
    let mut breath_hits: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    let mut last_present = 0u64;
    let mut buf = [0u8; 4096];
    let mut last_emit = Instant::now();
    // Broker fan-out socket (best-effort): relays vitals/feature packets so the
    // Health Monitor (and other vitals cogs) can still consume them while
    // presence-field owns 5006.
    let relay_sock = UdpSocket::bind("0.0.0.0:0").ok();
    loop {
        if let Ok((n, _)) = socket.recv_from(&mut buf) {
            let pkt = &buf[..n];
            if n >= 4 {
                let m = u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]);
                if m == VITALS_MAGIC || m == FUSED_MAGIC || m == FEATURE_MAGIC {
                    if let Some(rs) = &relay_sock { let _ = rs.send_to(pkt, &relay_addr); }
                }
            }
            if let Some((nid, amp)) = decode_csi(pkt) {
                if let Some(m) = base.nodes.get(&nid) {
                    let e = residual_energy(&amp, m);
                    let q = ring.entry(nid).or_default();
                    q.push_back(e);
                    while q.len() > window { q.pop_front(); }
                    // Breathing-band history: time-stamped residual, trimmed by age.
                    let now = Instant::now();
                    let h = hist.entry(nid).or_default();
                    h.push_back((now, e));
                    while h.front().is_some_and(|&(t, _)| now.duration_since(t) > Duration::from_secs(breath_secs)) {
                        h.pop_front();
                    }
                }
            }
        }
        if last_emit.elapsed() < Duration::from_secs(interval) { continue; }
        last_emit = Instant::now();

        // presence = max over nodes of (avg residual / empty floor)
        let mut best_ratio = 0.0f64;
        let mut per_node = serde_json::Map::new();
        for (nid, m) in &base.nodes {
            let avg = ring.get(nid).filter(|q| !q.is_empty())
                .map(|q| q.iter().sum::<f64>() / q.len() as f64).unwrap_or(0.0);
            let ratio = avg / m.floor;
            per_node.insert(format!("node{nid}"), serde_json::json!((ratio * 10.0).round() / 10.0));
            if ratio > best_ratio { best_ratio = ratio; }
        }
        // ── Breathing-band still-person detection ─────────────────────────
        // FFT each node's residual history in the respiration band; a stable,
        // physiological in-band peak with SNR over the band floor = a breathing
        // (still) person — caught where the motion ratio fades to baseline.
        let mut breath_bpm = 0.0f64;
        let mut breath_snr_best = 0.0f64;
        for nid in base.nodes.keys() {
            if let Some(h) = hist.get(nid) {
                if h.len() >= RESP_MIN_SAMPLES {
                    let span = h.back().unwrap().0.duration_since(h.front().unwrap().0).as_secs_f64();
                    if span > 1.0 {
                        let fs = (h.len() - 1) as f64 / span;
                        let series: Vec<f64> = h.iter().map(|&(_, e)| e).collect();
                        if let Some((bpm, snr)) = breathing_band(&series, fs) {
                            if snr > breath_snr_best { breath_snr_best = snr; breath_bpm = bpm; }
                        }
                    }
                }
            }
        }
        let breath_inband = breath_snr_best >= breath_snr
            && (RESP_LO_HZ * 60.0..=RESP_HI_HZ * 60.0).contains(&breath_bpm);
        // Temporal stability: require `breath_stable` consecutive in-band evals
        // whose rate is consistent (spread ≤ 4 bpm). Broadband house/AP/printer
        // noise has no stable in-band peak, so it never accumulates a streak.
        if breath_inband { breath_hits.push_back(breath_bpm); } else { breath_hits.clear(); }
        while breath_hits.len() > breath_stable { breath_hits.pop_front(); }
        let breath_present = breath_hits.len() >= breath_stable && {
            let mn = breath_hits.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = breath_hits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            mx - mn <= 4.0
        };

        let motion_detected = best_ratio > thresh;
        let detected = motion_detected || breath_present; // still-person OR moving
        let ts = now_secs();
        if detected { last_present = ts; }
        let present = detected || (ts.saturating_sub(last_present) < hold); // latch
        let method = if motion_detected { "motion-residual" }
            else if breath_present { "breathing-band" }
            else if present { "hold" }
            else { "none" };

        // Publish presence for downstream gating (Health Monitor reads this file).
        let pj = serde_json::json!({
            "present": present,
            "score": (best_ratio * 10.0).round() / 10.0,
            "method": method,
            "breathing_bpm": (breath_bpm * 10.0).round() / 10.0,
            "nodes": base.nodes.len(),
            "ts": ts,
        });
        let _ = std::fs::write(&presence_file, serde_json::to_vec(&pj).unwrap_or_default());

        let report = serde_json::json!({
            "presence_detected": present,
            "score": (best_ratio * 10.0).round() / 10.0,   // motion ratio over empty baseline
            "threshold": thresh,
            "per_node_ratio": per_node,
            "breathing_bpm": (breath_bpm * 10.0).round() / 10.0,
            "breathing_snr": (breath_snr_best * 10.0).round() / 10.0,
            "nodes": base.nodes.len(),
            "method": method,
            "timestamp": ts,
        });
        println!("{}", serde_json::to_string(&report).unwrap_or_default());
        store_presence(present, best_ratio);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq_hz: f64, fs: f64, secs: f64, amp: f64, dc: f64) -> Vec<f64> {
        let n = (fs * secs) as usize;
        (0..n)
            .map(|k| dc + amp * (2.0 * std::f64::consts::PI * freq_hz * k as f64 / fs).sin())
            .collect()
    }

    #[test]
    fn detects_physiological_breathing_sine() {
        // 0.25 Hz = 15 bpm, fs=8 Hz, 45 s — a clean respiration-rate signal.
        let s = sine(0.25, 8.0, 45.0, 1.0, 10.0);
        let (bpm, snr) = breathing_band(&s, 8.0).expect("clean in-band sine should detect");
        assert!((bpm - 15.0).abs() < 2.0, "bpm {bpm} not ~15");
        assert!(snr >= 4.0, "snr {snr} below gate for a clean sine");
    }

    #[test]
    fn rejects_flat_line() {
        // No modulation → no signal (an idle/saturated channel must not detect).
        assert!(breathing_band(&vec![10.0; 360], 8.0).is_none());
    }

    #[test]
    fn rejects_too_few_samples() {
        let s = sine(0.25, 8.0, 1.0, 1.0, 10.0); // ~8 samples
        assert!(breathing_band(&s, 8.0).is_none());
    }

    #[test]
    fn out_of_band_signal_is_not_a_physiological_peak() {
        // 1.5 Hz (90 "bpm") is above the respiration band; the in-band scan must
        // not report it as a physiological rate (≤ 30 bpm).
        let s = sine(1.5, 8.0, 45.0, 1.0, 10.0);
        if let Some((bpm, _snr)) = breathing_band(&s, 8.0) {
            assert!(bpm <= RESP_HI_HZ * 60.0 + 1.0, "out-of-band leaked as bpm {bpm}");
        }
    }

    #[test]
    fn freq_power_peaks_at_signal_frequency() {
        let fs = 8.0;
        let s = sine(0.3, fs, 40.0, 1.0, 0.0);
        let on = freq_power(&s, 0.3, fs);
        let off = freq_power(&s, 0.45, fs);
        assert!(on > off * 5.0, "power at signal freq ({on}) not dominant over off ({off})");
    }
}
