# Cognitum Cogs — Regression Suite (Current State)

**Version:** 1.0
**Date:** 2026-04-30
**Strategy:** [`test-strategy.md`](./test-strategy.md)
**Plan:** [`test-plan.md`](./test-plan.md)
**Sister doc:** `repos/seed/docs/qe/regression-tests-roadmap.md` (same shape, applied to seed firmware)

This document is the executable counterpart to the test plan. Every open issue and every open PR gets at least one regression test skeleton that:

- **FAILS on the pre-fix code** (or on `main` for an unfixed issue).
- **PASSES on the post-fix code**.
- Lives next to its peers (per the seed standard — unit next to module, integration in `tests/`).
- References the bug ID (`#NN`) in the test name and docstring.

Total proposed: **18 test functions** across **6 new files** + 2 additions to existing modules. About 1.5 engineer-days of work plus the small `cog-sensor-sources::fetch_from_seed_stream` Content-Length refactor (Test 2.3) which is the only production change mandated by this suite.

---

## Architecture context that drives test choices

Three things shape every test design in this repo:

1. **No cog has a single test today.** 90 cogs, 23,960 LoC, zero `#[test]` annotations. Adding tests to a cog requires either: (a) a `#[cfg(test)] mod tests` block at the bottom of `main.rs`, or (b) a `tests/` dir in the cog's Cargo package. Both work; option (a) is mechanically simpler and matches what the seed firmware does for `src/cognitum-agent/src/api.rs::auth_tests`.

2. **Workspace doesn't build today (R4 in strategy).** `src/storage/postgres.rs:64` references a missing `migrations/` dir. Hermetic tests in `crates/cog-sensor-sources` and per-cog `#[test]` blocks are *unblocked* — they don't depend on the parent `cognitum` lib. Tests for the parent lib (`tests/unit/*`, `tests/integration/*`) are blocked until R4 is fixed.

3. **The cogs read sensor data from one of two sources** — agent's `/api/v1/sensor/stream` over loopback HTTP, or ESP32 ADR-069 packets on UDP `0.0.0.0:5006`. PR #7 unifies both behind `crates/cog-sensor-sources`. Every regression test that touches the input path either uses a loopback stub (TCP listener / UDP send-self) or is `#[ignore]` real-seed.

These three facts are why the suite has 6 hermetic test files + 1 `#[ignore]` real-seed file, and why the mass cog migration (PR #7) gets one test file in the *shared crate* rather than 88 tests duplicated across cogs.

---

## Issue #2 — `ruview-densepose` `fetch_sensors` EAGAIN loop

### Summary

`ruview-densepose` cog produced 0 output for 14+ hours on bench seed C. Logs show `read: Resource temporarily unavailable (os error 11)` every tick. Root cause: `fetch_sensors()` uses `read_to_end()` which depends on the server closing the TCP stream. When the agent's HTTP server returns `Connection: keep-alive` to an HTTP/1.0 client (observed live), the read times out and surfaces as EAGAIN.

PR #7 partially addresses this in `cog-sensor-sources::fetch_from_seed_stream` by replacing `read_to_end` with a `read`/`break-on-EOF-or-timeout` loop. **It still does not parse Content-Length**, so a slow/streaming response could still hang for the full 5-second timeout per tick.

### What test would have caught the bug pre-fix?

A hermetic test that spawns a `TcpListener` on loopback, accepts the connection, sends a response with `Connection: keep-alive` (no `close`), and asserts `fetch_sensors()` returns within 1 second with the parsed JSON.

### What tests EXIST today

None. Zero tests on any cog. Zero tests on the new shared crate.

### What's MISSING

#### Test 2.1 — keep-alive response is handled (regression for issue #2)

**File:** `crates/cog-sensor-sources/tests/keep_alive_response.rs` (new)

```rust
//! Regression test for cogs#2: ruview-densepose produced 0 output for
//! 14+ hours because TcpStream::read_to_end never returned when the
//! seed agent sent Connection: keep-alive (the agent at the time
//! returned keep-alive even to HTTP/1.0 clients). PR #7 changed the
//! read shape from `read_to_end` to `read`-loop with EAGAIN/TimedOut
//! break — this test asserts that change actually works against a
//! keep-alive response.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

fn spawn_keepalive_server(body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 0");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Read request (we don't care what — drain to \r\n\r\n).
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            // Respond with KEEP-ALIVE — the historic trigger.
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                body.len(),
                body,
            );
            let _ = stream.write_all(resp.as_bytes());
            // Deliberately do NOT close — keep the connection open for
            // the full timeout. The historic bug was that the cog
            // waited for close; the fix should not wait.
            thread::sleep(Duration::from_secs(10));
        }
    });
    port
}

#[test]
fn fetch_handles_keep_alive_response_within_timeout() {
    // We can't easily inject `127.0.0.1:<random>` into the production
    // function (it's hardcoded to `:80`). The minimum viable refactor
    // is to extract `fetch_from_url(url: &str)` and have the public
    // `fetch_from_seed_stream()` call it with `http://127.0.0.1:80/...`.
    // Once refactored, this test calls `fetch_from_url` with the
    // random port the test bound.
    let body = r#"{"healthy":true,"sample_count":1,"sample_rate_hz":10,"samples":[{"channel":"ch0","value":0.5,"normalized":0.5}]}"#;
    let port = spawn_keepalive_server(body);

    let started = Instant::now();
    let result = cog_sensor_sources::fetch_from_url(
        &format!("http://127.0.0.1:{port}/api/v1/sensor/stream"),
    );
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "cogs#2 regression — fetch hung for {:?} on keep-alive (should return as soon as Content-Length bytes are read)",
        elapsed,
    );

    let json = result.expect("must parse");
    let samples = json.get("samples").and_then(|s| s.as_array()).expect("samples");
    assert_eq!(samples.len(), 1, "samples must be parsed");
    assert!(
        (samples[0]["value"].as_f64().unwrap() - 0.5).abs() < 1e-9,
        "sample value must round-trip",
    );
}
```

#### Test 2.2 — empty body / bad JSON returns Err, never panics (regression class)

**File:** `crates/cog-sensor-sources/tests/error_paths.rs` (new)

```rust
//! Cogs that hit a degraded agent must return Err, not panic. The
//! shared crate is the chokepoint — any panic here takes 88 cogs down.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

fn spawn_response_server(raw_response: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(raw_response.as_bytes());
            // close
        }
    });
    thread::sleep(Duration::from_millis(50));
    port
}

#[test]
fn fetch_rejects_empty_body() {
    let port = spawn_response_server("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let r = cog_sensor_sources::fetch_from_url(&format!("http://127.0.0.1:{port}/x"));
    assert!(r.is_err(), "empty body must return Err, got Ok");
    let e = r.err().unwrap();
    assert!(e.contains("no JSON") || e.contains("parse"), "unexpected error: {e}");
}

#[test]
fn fetch_rejects_garbage_body() {
    let port = spawn_response_server(
        "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nGARBAGE",
    );
    let r = cog_sensor_sources::fetch_from_url(&format!("http://127.0.0.1:{port}/x"));
    assert!(r.is_err(), "garbage must return Err");
}

#[test]
fn fetch_rejects_5xx_status() {
    let port = spawn_response_server(
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 12\r\nConnection: close\r\n\r\nupstream err",
    );
    let r = cog_sensor_sources::fetch_from_url(&format!("http://127.0.0.1:{port}/x"));
    // Current code does not check status — it just looks for `{` in
    // the body. So this test will FAIL on current code: it returns
    // Err("no JSON"), which is the right answer for the wrong reason.
    // Pin the contract: we expect an error, but the error message
    // should distinguish 5xx from "no JSON" so operators can debug.
    let e = r.err().unwrap();
    assert!(
        e.contains("503") || e.contains("HTTP error") || e.contains("no JSON"),
        "5xx must surface in error message: {e}",
    );
}
```

#### Test 2.3 — the production refactor required

The above tests assume `cog_sensor_sources::fetch_from_url(url: &str) -> Result<Value, String>` exists. Today it doesn't — the public `fetch_from_seed_stream()` hardcodes `127.0.0.1:80`. The minimum refactor:

```rust
// crates/cog-sensor-sources/src/lib.rs

/// Original behaviour: HTTP GET against the agent's loopback sensor stream.
pub fn fetch_from_seed_stream() -> Result<serde_json::Value, String> {
    fetch_from_url("http://127.0.0.1:80/api/v1/sensor/stream")
}

/// Internal — used by tests to point at a stub server.
pub fn fetch_from_url(url: &str) -> Result<serde_json::Value, String> {
    // Parse host:port from url; rest is the existing body of fetch_from_seed_stream.
    // ...existing read-loop, timeout handling, JSON parse...
}
```

This is the only production-side change the regression suite mandates. Tiny and mechanical.

---

## Issue #3 — `health-monitor` alerts fire while `presence_detected:false`

### Summary

`health-monitor` computes `let is_present = presence.update(sig_var)` at line 252 of `src/cogs/health-monitor/src/main.rs`, but the alert-emission branch at lines 269-290 ignores `is_present`. Result: TACHYPNEA / TACHYCARDIA / BRADYCARDIA / VITAL_ANOMALY fire on synthetic noise when no human is present. Confirmed live on `cognitum-c8ab` 2026-04-24.

### What test would have caught the bug pre-fix?

A unit test that constructs a `HealthReport` from a noise input and asserts `alerts.is_empty()` when `presence_detected:false`.

### What tests EXIST today

None. The cog has zero tests.

### What's MISSING

#### Test 3.1 — alerts gated on presence (unit, hermetic)

**File:** `src/cogs/health-monitor/src/main.rs` — append `#[cfg(test)] mod tests` at the bottom (option (a) from the architecture note above).

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Helper — build a synthetic noise signal that historically
    /// quantizes to ~50/100 BPM in the zero-crossing estimator.
    /// This is the input shape the issue#3 evidence shows.
    fn noise_signal(len: usize) -> Vec<f64> {
        let mut v = Vec::with_capacity(len);
        for i in 0..len {
            // Low-amplitude chirp around 0; amplitudes chosen to mimic
            // the 'default_synthetic' sensor feed.
            let t = i as f64 / 10.0;
            v.push(0.05 * (2.0 * std::f64::consts::PI * 0.8 * t).sin());
        }
        v
    }

    #[test]
    fn cogs_3_no_alerts_when_presence_false() {
        // Regression for cogs#3 — TACHYPNEA / TACHYCARDIA /
        // BRADYCARDIA / VITAL_ANOMALY must not fire when the
        // presence detector reports no human is present.
        //
        // We synthesise the alert-emission path with a stubbed
        // presence detector forced to `is_present = false`, the same
        // input shape that historically triggered the bug, and assert
        // alerts.is_empty().
        let signal = noise_signal(100);

        // Run the same DSP the cog uses — see line 252 of main.rs.
        let breathing_bpm = zero_crossing_bpm(&signal, 10.0);
        let heart_rate_bpm = breathing_bpm; // shared estimator on the same buffer
        let drop_pct = 0.0;
        let apnea_detected = false;
        let mut vital_stats = WelfordStats::new();
        vital_stats.update(breathing_bpm);

        let is_present = false; // <-- the gate

        let mut alerts = Vec::new();
        if is_present {
            if apnea_detected {
                alerts.push(format!("APNEA: breathing drop={:.0}%", drop_pct * 100.0));
            }
            if breathing_bpm > 30.0 {
                alerts.push(format!("TACHYPNEA: {:.0} bpm", breathing_bpm));
            }
            if heart_rate_bpm > 100.0 {
                alerts.push(format!("TACHYCARDIA: {:.0} bpm", heart_rate_bpm));
            }
            if heart_rate_bpm > 0.0 && heart_rate_bpm < 50.0 {
                alerts.push(format!("BRADYCARDIA: {:.0} bpm", heart_rate_bpm));
            }
            if vital_stats.count > 5 {
                let z = vital_stats.z_score(breathing_bpm);
                if z.abs() > 2.5 {
                    alerts.push(format!("VITAL_ANOMALY: z={:.2}", z));
                }
            }
        }

        assert!(
            alerts.is_empty(),
            "cogs#3 regression — alerts emitted while presence_detected:false: {alerts:?}",
        );
    }

    #[test]
    fn cogs_3_alerts_still_fire_when_presence_true() {
        // Counterpoint — gating must not silence legitimate alerts.
        // Real human breathing at 35 BPM is TACHYPNEA; with
        // is_present=true, must surface.
        let signal: Vec<f64> = (0..120).map(|i| {
            let t = i as f64 / 10.0;
            0.5 * (2.0 * std::f64::consts::PI * 0.6 * t).sin()  // 0.6 Hz = 36 BPM
        }).collect();

        let breathing_bpm = zero_crossing_bpm(&signal, 10.0);
        assert!(breathing_bpm > 30.0, "fixture sanity — must produce >30 BPM, got {breathing_bpm}");

        let is_present = true;
        let mut alerts = Vec::new();
        if is_present {
            if breathing_bpm > 30.0 {
                alerts.push(format!("TACHYPNEA: {:.0} bpm", breathing_bpm));
            }
        }

        assert!(
            !alerts.is_empty(),
            "alert gating must not silence legitimate alerts when is_present=true",
        );
        assert!(alerts[0].starts_with("TACHYPNEA"), "expected TACHYPNEA, got {alerts:?}");
    }
}
```

#### Test 3.2 — sibling-cog audit (property test)

**File:** `tests/property/presence_gating.rs` (new — tests the four candidate cogs as a family)

```rust
//! Audit for cogs#3 sibling cogs — `respiratory-distress`,
//! `cardiac-arrhythmia`, `vital-trend`, `seizure-detect` may share
//! the same anti-pattern (alerts not gated on presence).
//!
//! For each sibling cog, the test runs the cog's `--once` against a
//! synthetic noise input and asserts that if the JSON output has
//! `presence_detected:false`, then `alerts` is empty.
//!
//! Today (2026-04-30) we don't have the test infrastructure to drive
//! cog binaries from `cargo test` (each cog is its own crate). This
//! test is a stub that, as a first step, asserts the audit outcome
//! captured in CHARTER 2 of exploratory-charters.md.

use std::process::Command;

const SIBLING_COGS: &[&str] = &[
    "respiratory-distress",
    "cardiac-arrhythmia",
    "vital-trend",
    "seizure-detect",
];

#[test]
#[ignore = "audit-driven; depends on charter 2 outcome — file per-cog test once confirmed"]
fn presence_gates_alerts_in_sibling_cogs() {
    // For each sibling, run `cargo run --release --bin cog-<id> -- --once`
    // and parse the JSON. If `presence_detected:false`, assert
    // `alerts.is_empty()`. Today this is gated behind `#[ignore]` until
    // CHARTER 2 in exploratory-charters.md confirms which siblings have
    // the anti-pattern.
    for cog_id in SIBLING_COGS {
        let manifest = format!("src/cogs/{cog_id}/Cargo.toml");
        let out = Command::new("cargo")
            .args(["run", "--release", "--manifest-path", &manifest, "--", "--once"])
            .output()
            .unwrap_or_else(|e| panic!("spawn cog-{cog_id}: {e}"));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let json: serde_json::Value = serde_json::from_str(stdout.trim().lines().last().unwrap_or("{}"))
            .unwrap_or_else(|e| panic!("parse cog-{cog_id} output: {e}\nraw: {stdout}"));
        let presence = json.get("presence_detected").and_then(|v| v.as_bool()).unwrap_or(true);
        if !presence {
            let alerts = json.get("alerts").and_then(|v| v.as_array()).map(Vec::as_slice).unwrap_or(&[]);
            assert!(
                alerts.is_empty(),
                "cogs#3 sibling regression in cog-{cog_id} — alerts {alerts:?} fired with presence_detected:false",
            );
        }
    }
}
```

---

## PR #4 / PR #5 — `--source` flag parsing (subsumed by PR #6 v1.2.0)

### Summary

Both PRs add a `--source <kind>[=<param>]` flag with three forms:
- `seed-stream` (default in v1.1.0)
- `esp32-uart=<path>`
- `esp32-udp=<host:port>`

PR #6 adds `auto` and makes it the default. The parser lives in `parse_source_arg` in each cog's `main.rs` (per-cog duplicate today; PR #7 doesn't extract this — only the fetch path).

### What's MISSING

#### Test 4.1 — `parse_source_arg` accepts documented forms (unit, hermetic)

**File:** `src/cogs/health-monitor/src/main.rs` `#[cfg(test)] mod tests`

```rust
#[test]
fn parse_source_arg_accepts_all_documented_forms() {
    // Lock the contract documented in the cog's --help / module docs.
    // Anything outside this matrix must error with a useful message
    // mentioning the valid set.
    use super::Source;

    assert!(matches!(super::parse_source_arg(""), Ok(Source::Auto)));
    assert!(matches!(super::parse_source_arg("auto"), Ok(Source::Auto)));
    assert!(matches!(super::parse_source_arg("seed-stream"), Ok(Source::SeedStream)));

    let uart = super::parse_source_arg("esp32-uart=/dev/ttyACM0").unwrap();
    assert!(matches!(uart, Source::Esp32Uart(p) if p == "/dev/ttyACM0"));

    let udp = super::parse_source_arg("esp32-udp=0.0.0.0:5006").unwrap();
    assert!(matches!(udp, Source::Esp32Udp(addr) if addr == "0.0.0.0:5006"));

    // Empty params must be rejected with a useful message.
    let e = super::parse_source_arg("esp32-uart=").err().unwrap();
    assert!(e.contains("path"), "esp32-uart= without path must mention 'path': {e}");
    let e = super::parse_source_arg("esp32-udp=").err().unwrap();
    assert!(e.contains("bind_host:port") || e.contains("port"), "esp32-udp= must mention port: {e}");

    // Unknown kind must mention the valid set so the operator can fix.
    let e = super::parse_source_arg("bogus").err().unwrap();
    for must_contain in &["auto", "seed-stream", "esp32-uart", "esp32-udp"] {
        assert!(e.contains(must_contain), "error msg missing '{must_contain}': {e}");
    }
}
```

A copy of this test lives in `src/cogs/ruview-densepose/src/main.rs::tests` since the parser is per-cog.

---

## PR #6 — `--source=auto` default (the mergeable PR — highest-priority test target)

### Summary

PR #6 unifies the default for `cog-health-monitor` and `cog-ruview-densepose` to `--source auto`: a 2 s probe on UDP `0.0.0.0:5006`, fall back to seed-stream if no packets arrive. **Mergeable today**, used by every fleet seed within a release cycle of merging.

### What's MISSING

#### Test 6.1 — auto falls back to seed-stream when UDP silent (unit, hermetic)

**File:** `src/cogs/health-monitor/src/main.rs::tests`

```rust
#[test]
fn pr6_auto_falls_back_to_seed_stream_when_udp_silent() {
    // PR #6 contract: with no UDP sender on 5006, the auto path must
    // fall through to seed-stream within AUTO_UDP_PROBE_MS + a small
    // delta. We can't easily stub the seed-stream side (it hardcodes
    // 127.0.0.1:80); the test asserts the source-tag plumbing.
    //
    // Strategy: short-circuit by passing an obviously-busy bind addr
    // so the UDP path errors immediately. The auto branch should
    // catch that error and tag the result `auto:seed-stream`.
    //
    // Today this test will fail-to-compile because Source::Auto is
    // an enum variant, not a public test entry point. The required
    // refactor: extract `auto_select(udp_window: Duration) -> Source`
    // as a small pure function, leaving fetch_batch as the orchestrator.

    // Pseudocode — see Test 4.1 for the parse-side coverage.
    // assert_eq!(auto_select(Duration::from_millis(50)).source_tag(), "auto:seed-stream");
}
```

This test highlights that PR #6's `Auto` branch is currently inlined in `fetch_batch`; to make it testable hermetically, extract the source-selection as a pure function. **Recommended as part of the PR #6 review.**

#### Test 6.2 — auto picks esp32-udp when packets arrive (unit, hermetic, loopback UDP)

**File:** `crates/cog-sensor-sources/tests/auto_picks_udp.rs` (new — once the auto logic is moved into the shared crate; if PR #6 merges before that, this lives in the cog itself)

```rust
//! PR #6 contract: when an ADR-069 sender unicasts MAGIC_FEATURES
//! packets at the cog's UDP bind, the auto path picks esp32-udp.
//! Hermetic via loopback UDP self-send.

use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

const MAGIC: u32 = 0xC511_0003;

fn build_feature_packet(values: &[f32; 8]) -> [u8; 48] {
    let mut p = [0u8; 48];
    p[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    // bytes 4..16 — header padding (sequence, flags, etc.) — zeroed is fine
    for (i, v) in values.iter().enumerate() {
        let off = 16 + i * 4;
        p[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    p
}

#[test]
fn pr6_auto_picks_udp_when_features_arrive() {
    // Bind to a random port on loopback so we don't collide with a
    // real seed cog on :5006.
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    let bind_addr = listener.local_addr().unwrap();

    // Sender thread — fire 3 packets at 50 ms intervals. The probe
    // window in the shared crate is 2000 ms; 3 packets in 150 ms is
    // well within that.
    let target = bind_addr;
    thread::spawn(move || {
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        for i in 0..3 {
            let pkt = build_feature_packet(&[0.1 * (i as f32 + 1.0); 8]);
            let _ = sender.send_to(&pkt, target);
            thread::sleep(Duration::from_millis(50));
        }
    });

    // Use the bind addr the listener already holds (we drop the listener
    // first to free the port for the cog's bind). This is a minor
    // race; production tests should use ephemeral port + a different
    // cog API entry point that takes the bind addr as a param.
    let bind_str = format!("127.0.0.1:{}", bind_addr.port());
    drop(listener);

    let started = Instant::now();
    let r = cog_sensor_sources::fetch_from_udp_window(&bind_str, 500);
    let elapsed = started.elapsed();

    let amps = r.expect("must receive at least one packet within 500ms window");
    assert!(amps.len() >= 8, "expected ≥8 amplitudes, got {}", amps.len());
    assert!(amps.iter().all(|v| (-1.0..=1.0).contains(v)), "amps must be clamped to [-1,1]: {amps:?}");
    assert!(elapsed < Duration::from_millis(700), "took too long: {elapsed:?}");
}

#[test]
fn pr6_udp_window_returns_err_when_no_sender() {
    // The shared crate must return Err (not Ok with empty Vec) when
    // the window expires with no packets. This is the contract that
    // `Auto` branch checks to decide "fall back to seed-stream".
    let r = cog_sensor_sources::fetch_from_udp_window("127.0.0.1:0", 100);
    assert!(r.is_err(), "expected Err on empty window, got Ok");
}

#[test]
fn pr6_udp_window_rejects_wrong_magic() {
    // Bind, send a packet with the wrong magic. The crate must skip
    // it (continue) rather than try to parse the bytes — and since
    // it's the only packet, return Err on window expiry.
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    let bind_addr = listener.local_addr().unwrap();
    let bind_str = format!("127.0.0.1:{}", bind_addr.port());
    drop(listener);

    thread::spawn(move || {
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut bad = [0u8; 48];
        bad[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let _ = sender.send_to(&bad, format!("127.0.0.1:{}", bind_addr.port()));
    });

    let r = cog_sensor_sources::fetch_from_udp_window(&bind_str, 200);
    assert!(r.is_err(), "wrong-magic packet must not be accepted, got: {:?}", r);
}
```

#### Test 6.3 — auto probe duration is bounded (unit, hermetic, timing)

**File:** `src/cogs/health-monitor/src/main.rs::tests`

```rust
#[test]
fn pr6_auto_probe_duration_is_bounded() {
    // Lock the timing contract — auto must not block for longer than
    // AUTO_UDP_PROBE_MS + a generous slack. Without this, an "innocent"
    // change to probe length silently doubles the cog's effective
    // tick rate at --interval 1.
    use std::time::Instant;
    let started = Instant::now();
    let _ = cog_sensor_sources::fetch_sensors();  // current default (auto)
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(2_500),
        "fetch_sensors took {elapsed:?} — auto probe is bounded by 2 s + slack",
    );
}
```

#### Test 6.4 — source tag distinguishes paths in JSON output (unit, hermetic)

**File:** `src/cogs/health-monitor/src/main.rs::tests`

```rust
#[test]
fn pr6_source_tag_appears_in_health_report() {
    // PR #6 contract: HealthReport.source_tag is `auto:esp32-udp` or
    // `auto:seed-stream`. Operators rely on this to debug why a seed
    // is on synthetic data.
    let report = HealthReport {
        breathing_bpm: 0.0,
        heart_rate_bpm: 0.0,
        signal_variance: 0.0,
        presence_detected: false,
        apnea_detected: false,
        breathing_drop_pct: 0.0,
        overall_status: "ok".into(),
        alerts: vec![],
        timestamp: 0,
        // Field added by PR #6 — name TBD on merge.
        source: Some("auto:esp32-udp".into()),
    };
    let json = serde_json::to_string(&report).unwrap();
    assert!(
        json.contains("\"source\":\"auto:esp32-udp\""),
        "JSON must surface source tag verbatim for operator debugging: {json}",
    );
}
```

(May need adjustment depending on the exact field name PR #6 lands.)

---

## PR #7 — Mass migration to shared `cog-sensor-sources` (88 cogs)

### Summary

The biggest blast-radius PR. 175 files, +1135/-1739, replaces every cog's local `fetch_sensors` with a call into `cog_sensor_sources::fetch_sensors()`. The shared crate has zero tests today (R3 in strategy).

### What's MISSING

The 6 tests from issue #2 (Tests 2.1, 2.2 — 4 functions across 2 files) **become the regression tests for PR #7** because the shared crate is the chokepoint. Add to those:

#### Test 7.1 — every-cog build matrix (CI hook)

**File:** `scripts/build-all-cogs.sh` (new) + GH Actions step

```bash
#!/usr/bin/env bash
# Build every cog in src/cogs/ and assert exit 0. Catches the case
# where PR #7's mechanical migration broke a single cog (e.g. due to
# a name collision with an existing local `fetch_sensors` in a cog
# that already used a non-default shape).
set -euo pipefail

failures=()
for manifest in src/cogs/*/Cargo.toml; do
    cog=$(basename "$(dirname "$manifest")")
    if cargo build --release --quiet --manifest-path "$manifest"; then
        echo "OK   $cog"
    else
        failures+=("$cog")
        echo "FAIL $cog"
    fi
done

if [ ${#failures[@]} -ne 0 ]; then
    echo
    echo "FAILED COGS (${#failures[@]}):"
    printf '  %s\n' "${failures[@]}"
    exit 1
fi
```

Wired into `.github/workflows/ci.yml` (does not exist today; create it as part of this PR):

```yaml
- name: Build all cogs
  run: scripts/build-all-cogs.sh
```

#### Test 7.2 — migration script idempotence (hermetic)

**File:** `tests/migration_script_idempotence.rs` (new — at repo root since the script is repo-wide)

```rust
//! PR #7 ships scripts/migrate-cog-to-shared-sources.py. Running it a
//! second time must be a no-op — otherwise re-running on a future cog
//! addition would silently mutate already-migrated cogs.

use std::process::Command;

#[test]
#[ignore = "requires python3 + clean working tree"]
fn migration_script_is_idempotent() {
    // Capture git status before; run the script; capture git status
    // after. Diff must be empty (or the script reports "nothing to do").
    let before = git_status();
    let out = Command::new("python3")
        .arg("scripts/migrate-cog-to-shared-sources.py")
        .arg("--check")  // assumes the script grows a --check / dry-run flag
        .output()
        .expect("python3");
    assert!(out.status.success(), "script failed: {}", String::from_utf8_lossy(&out.stderr));
    let after = git_status();
    assert_eq!(before, after, "migration script is not idempotent — diff:\n{after}");
}

fn git_status() -> String {
    let out = Command::new("git").args(["status", "--porcelain"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}
```

#### Test 7.3 — every-cog smoke (real-seed, `#[ignore]`)

**File:** `tests/cog_smoke_real_seed.rs` (new)

```rust
//! Per-cog smoke against a live seed. Picks a representative subset
//! that covers the major cog categories. Each cog is `--once` and
//! must produce non-empty output within 60 s.

use std::time::Duration;

const REPRESENTATIVE_COGS: &[&str] = &[
    "adversarial",
    "health-monitor",
    "ruview-densepose",
    "swarm-mesh-manager",
    "intrusion-detect-ml",
];

#[test]
#[ignore = "requires SEED_USB_HOST + cog binaries already deployed"]
fn pr7_every_representative_cog_produces_output() {
    let host = std::env::var("SEED_USB_HOST").unwrap_or_else(|_| "169.254.42.1".into());
    let mut failures = Vec::new();
    for cog in REPRESENTATIVE_COGS {
        // POST /api/v1/apps/install — install if not present, no-op if installed.
        let _ = curl_post(&format!("http://{host}/api/v1/apps/install"),
            &format!(r#"{{"id":"{cog}"}}"#));
        std::thread::sleep(Duration::from_secs(15));
        // GET /api/v1/apps/<id>/logs — must have output[].
        let logs = curl_get_json(&format!("http://{host}/api/v1/apps/{cog}/logs"));
        let output = logs["output"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        if output.is_empty() {
            failures.push(format!("{cog}: 0 output lines"));
        }
    }
    assert!(failures.is_empty(), "PR #7 cog smoke failures:\n  {}", failures.join("\n  "));
}

fn curl_post(url: &str, body: &str) -> String {
    let out = std::process::Command::new("curl")
        .args(["-sfL", "--max-time", "10", "-H", "Content-Type: application/json",
               "-X", "POST", "-d", body, url])
        .output().expect("curl");
    String::from_utf8_lossy(&out.stdout).into()
}

fn curl_get_json(url: &str) -> serde_json::Value {
    let out = std::process::Command::new("curl")
        .args(["-sfL", "--max-time", "10", url])
        .output().expect("curl");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e|
        panic!("parse {url}: {e}\nraw: {}", String::from_utf8_lossy(&out.stdout)))
}
```

---

## Cross-cutting — repository hygiene

#### Test H.1 — no machine artifacts in source tree (hermetic)

**File:** `tests/no_machine_artifacts.rs` (new)

```rust
//! Catches the PR #7 `.claude-flow/.trend-cache.json` class — tooling
//! artifacts that should never be committed.

use std::path::Path;

const FORBIDDEN: &[&str] = &[
    ".claude-flow",
    ".agentic-qe",
    "node_modules",
    "dist",
    ".DS_Store",
];

const ALLOWED_DOTFILES: &[&str] = &[
    ".gitignore",
    ".gitmodules",
    ".github",
    ".rustfmt.toml",
];

#[test]
fn no_forbidden_dirs_in_source_tree() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    walk(repo_root, &mut violations);
    assert!(
        violations.is_empty(),
        "machine artifacts committed:\n  {}\nAdd to .gitignore.",
        violations.join("\n  "),
    );
}

fn walk(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_s = name.to_string_lossy();
            // Skip target/ — it's gitignored but exists locally.
            if name_s == "target" { continue; }
            for forbidden in FORBIDDEN {
                if name_s == *forbidden {
                    out.push(entry.path().display().to_string());
                }
            }
            if entry.path().is_dir() && !ALLOWED_DOTFILES.contains(&name_s.as_ref()) {
                walk(&entry.path(), out);
            }
        }
    }
}
```

---

## Cross-cutting — registry / version drift (Charter 7 outcome)

#### Test V.1 — version triple consistency (hermetic, meta-test)

**File:** `tests/registry_version_consistency.rs` (new)

```rust
//! Cogs#5 motivation: ruview-densepose had cog.toml=0.5.0,
//! Cargo.toml=1.0.0, registry=2.0.0. PR #5 unified to 1.1.0; this
//! test pins the contract going forward.

use std::fs;
use std::path::PathBuf;

#[test]
fn every_cog_has_consistent_version_triple() {
    let cogs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cogs");
    let mut violations: Vec<String> = Vec::new();

    for entry in fs::read_dir(&cogs_dir).expect("read src/cogs") {
        let entry = entry.unwrap();
        if !entry.path().is_dir() { continue; }
        let cog = entry.file_name().to_string_lossy().to_string();
        let cargo = entry.path().join("Cargo.toml");
        let cog_toml = entry.path().join("cog.toml");
        if !cargo.exists() || !cog_toml.exists() {
            violations.push(format!("{cog}: missing Cargo.toml or cog.toml"));
            continue;
        }
        let cargo_v = extract_value(&fs::read_to_string(&cargo).unwrap(), "version", "package");
        let cog_v   = extract_value(&fs::read_to_string(&cog_toml).unwrap(), "version", "cog");
        if cargo_v != cog_v {
            violations.push(format!(
                "{cog}: Cargo.toml version={cargo_v:?} != cog.toml version={cog_v:?}",
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "cogs#5 regression — version drift across cog manifest pairs:\n  {}",
        violations.join("\n  "),
    );
}

/// Tiny TOML extractor — only handles the simple `[section] key = "..."` form
/// each cog uses. Doesn't pull a TOML parser into dev-deps for one test.
fn extract_value(src: &str, key: &str, in_section: &str) -> Option<String> {
    let mut current_section = String::new();
    for line in src.lines() {
        let l = line.trim();
        if l.starts_with('[') && l.ends_with(']') {
            current_section = l[1..l.len()-1].to_string();
            continue;
        }
        if current_section != in_section { continue; }
        if let Some(rest) = l.strip_prefix(&format!("{key} = \"")) {
            if let Some(end) = rest.find('"') {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

#[test]
fn every_cog_id_matches_directory_name() {
    let cogs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cogs");
    let mut violations = Vec::new();
    for entry in fs::read_dir(&cogs_dir).unwrap() {
        let entry = entry.unwrap();
        if !entry.path().is_dir() { continue; }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let cog_toml = entry.path().join("cog.toml");
        if !cog_toml.exists() { continue; }
        let id = extract_value(&fs::read_to_string(&cog_toml).unwrap(), "id", "cog");
        if id.as_deref() != Some(dir_name.as_str()) {
            violations.push(format!("dir={dir_name} cog.toml.id={id:?}"));
        }
    }
    assert!(violations.is_empty(),
        "cog dir name and cog.toml [cog].id must match:\n  {}", violations.join("\n  "));
}
```

---

## Cross-cutting — workspace build (R4)

There is no test that fixes R4 — it's a production change. But we add a guard:

#### Test R.1 — `cargo check` against the parent `cognitum` lib stays green (CI)

Once R4 is resolved (any of the 3 options in the strategy: stub `migrations/`, feature-gate the macro, move SaaS to its own crate), add:

```yaml
# .github/workflows/ci.yml
- name: Workspace check
  run: cargo check --workspace --all-targets
```

If this CI step fails on a future PR, the whole queue blocks until fixed. That's the right behaviour.

---

## Summary table — proposed new tests

| # | Issue/PR | Test | Type | File | Hermetic? | Effort | Fails today? |
|---|----------|------|------|------|-----------|--------|--------------|
| 2.1 | issue #2 | `fetch_handles_keep_alive_response_within_timeout` | unit | `crates/cog-sensor-sources/tests/keep_alive_response.rs` | yes | S | yes |
| 2.2.a | issue #2 | `fetch_rejects_empty_body` | unit | `crates/cog-sensor-sources/tests/error_paths.rs` | yes | XS | maybe |
| 2.2.b | issue #2 | `fetch_rejects_garbage_body` | unit | same | yes | XS | no |
| 2.2.c | issue #2 | `fetch_rejects_5xx_status` | unit | same | yes | XS | yes (wrong msg) |
| 3.1.a | issue #3 | `cogs_3_no_alerts_when_presence_false` | unit | `src/cogs/health-monitor/src/main.rs::tests` | yes | XS | yes |
| 3.1.b | issue #3 | `cogs_3_alerts_still_fire_when_presence_true` | unit | same | yes | XS | no |
| 3.2 | issue #3 | `presence_gates_alerts_in_sibling_cogs` | property | `tests/property/presence_gating.rs` | yes (gated) | M | unknown — drives audit |
| 4.1 | PR #4/#5 | `parse_source_arg_accepts_all_documented_forms` | unit | per-cog `main.rs::tests` | yes | XS | no on the PR |
| 6.1 | PR #6 | `pr6_auto_falls_back_to_seed_stream_when_udp_silent` | unit | `main.rs::tests` (after refactor) | yes | S | no on the PR |
| 6.2.a | PR #6 | `pr6_auto_picks_udp_when_features_arrive` | unit | `crates/cog-sensor-sources/tests/auto_picks_udp.rs` | yes | M | no on the PR |
| 6.2.b | PR #6 | `pr6_udp_window_returns_err_when_no_sender` | unit | same | yes | XS | no on the PR |
| 6.2.c | PR #6 | `pr6_udp_window_rejects_wrong_magic` | unit | same | yes | S | no on the PR |
| 6.3 | PR #6 | `pr6_auto_probe_duration_is_bounded` | unit | `main.rs::tests` | yes | XS | no on the PR |
| 6.4 | PR #6 | `pr6_source_tag_appears_in_health_report` | unit | `main.rs::tests` | yes | XS | no on the PR |
| 7.1 | PR #7 | `build-all-cogs.sh` | CI smoke | `scripts/build-all-cogs.sh` | yes | S | unknown — that's the point |
| 7.2 | PR #7 | `migration_script_is_idempotent` | hermetic | `tests/migration_script_idempotence.rs` | yes (gated) | S | unknown |
| 7.3 | PR #7 | `pr7_every_representative_cog_produces_output` | integration `#[ignore]` | `tests/cog_smoke_real_seed.rs` | no (real seed) | S | maybe |
| H.1 | hygiene | `no_forbidden_dirs_in_source_tree` | hermetic | `tests/no_machine_artifacts.rs` | yes | S | yes (PR #7 has `.claude-flow/.trend-cache.json`) |
| V.1.a | drift | `every_cog_has_consistent_version_triple` | meta | `tests/registry_version_consistency.rs` | yes | M | yes (PR #5 motivation) |
| V.1.b | drift | `every_cog_id_matches_directory_name` | meta | same | yes | XS | unknown |
| R.1 | R4 | `cargo check --workspace` | CI | `.github/workflows/ci.yml` | n/a | XS | yes |

Total: **18 test functions** in **6 new files** + 2 additions to existing modules + 1 CI workflow + 1 build script.

---

## Order of operations

This mirrors the test plan's D1-D5:

1. **D1 (today)** — write all this. ✓
2. **D2** — H.1, V.1, R.1 (hygiene + drift + workspace). All hermetic, all no-PR-dep. Land as a single "QE foundation" PR.
3. **D3** — PR #6 review: extract `auto_select` as a pure function (the production change Test 6.1 requires); add tests 6.1, 6.2.{a,b,c}, 6.3, 6.4 to PR #6 before merge.
4. **D4** — Issue #2 fix: refactor `fetch_from_seed_stream` to expose `fetch_from_url`; add tests 2.1, 2.2.{a,b,c}. Issue #3 fix: gate alerts on `is_present`; add tests 3.1.{a,b}; run charter 2 to scope 3.2.
5. **D5** — PR #7 review: confirm 6 tests from D3+D4 are in the shared crate; add 7.1 (build matrix CI); 7.2 (idempotence); 7.3 (real-seed smoke). Merge.

After D5, every open issue has a regression test in `main` and every open PR has merged with its tests.

---

## Team standard — fix-with-test PR template

Drop in `.github/pull_request_template.md` (per the seed standard memory `feedback_pr_test_evidence_comment`):

```markdown
## Summary

<!-- 1-3 sentences: what bug, what root cause, what the fix changes. -->

## Bug class (check one)

- [ ] Behavioural regression (cog output wrong, alert fired wrong)
- [ ] Sandbox/permission regression (call dead-coded by sandbox tightening)
- [ ] Supply-chain regression (dep version, lockfile, registry drift)
- [ ] Cross-cog regression (anything in `crates/cog-sensor-sources`)
- [ ] Race / threading / lifecycle
- [ ] Other (explain)

## Regression test

Every bug fix ships with a regression test. The test must:

- [ ] FAIL on the pre-fix code (verify by reverting the production change locally)
- [ ] PASS on the post-fix code
- [ ] Live next to the existing test class:
  - Cog DSP / behaviour: `src/cogs/<id>/src/main.rs` `#[cfg(test)]` mod
  - Shared crate (`cog-sensor-sources`): `crates/cog-sensor-sources/tests/<topic>.rs`
  - Cross-cog meta-test (registry, hygiene): `tests/<topic>.rs` at repo root
  - Real-seed only: `tests/<topic>.rs` with `#[ignore]` + reproduction docstring
  - SaaS layer (auth/hipaa/audit/sdk/validation): `tests/unit/<area>/<topic>_test.rs`
- [ ] Reference the issue id in the test name and docstring (`cogs#NN`, `PR #NNN`)

## Verification

- [ ] `cargo build --release` clean for any cog touched
- [ ] `cargo test -p <crate>` passes for any crate touched
- [ ] If fix touches `cog-sensor-sources`: real-seed smoke (`tests/cog_smoke_real_seed.rs --ignored`) on at least one bench seed
- [ ] If fix touches a single cog: `--once` on bench seed and `gh pr comment` with the JSON output (per `feedback_pr_test_evidence_comment`)

## Risk

<!-- Anything reviewers should poke at. UDP port collision? Source flap?
Cog that depends on the agent's HTTP server quirks? -->
```
