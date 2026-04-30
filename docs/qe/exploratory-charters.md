# Cognitum Cogs — Exploratory Test Charters

**Version:** 1.0
**Date:** 2026-04-30
**Format:** Session-Based Test Management (SBTM) per James Bach.
**Strategy:** [`test-strategy.md`](./test-strategy.md)

Each charter is a 60-90 min focused exploration with a clear mission, areas to attack, and what kind of bug pattern to look for. Charters are NOT scripted — they ARE bounded.

Format per charter:
```
CHARTER N — <one-line mission>
Mission     :  what you're trying to learn
Areas       :  HTSM SFDIPOT factor(s), files/modules
Time        :  T (target session length, minutes)
Setup       :  what you need before starting
Tactics     :  heuristics / tour types to use
Stop when   :  exit criteria
Risks       :  what kinds of bugs you expect to find
Debrief     :  what to capture in the session report
```

Active sessions get a corresponding `docs/qe/sessions/<charter-id>-<date>.md` report after completion.

---

## CHARTER 1 — `cog-sensor-sources::fetch_sensors` under abnormal HTTP responses

**Mission:** Find the input shapes that make the new `fetch_from_seed_stream` (PR #7) hang, error wrong, or return malformed data — beyond what the issue #2 fix anticipated.

**Areas:** D (Data), I (Interfaces). Files: `crates/cog-sensor-sources/src/lib.rs:97-130`.

**Time:** 90 min.

**Setup:**
- Local checkout of `feat/cogs-shared-sources-all` branch.
- Ability to run a TCP listener on a chosen loopback port (`nc -l 8080`, or a small Rust harness in `tests/`).
- The cog binary built with a tiny driver `examples/sensor_source_probe.rs` that just calls `fetch_sensors()` once and prints the result + duration.

**Tactics:**
- **Hostile-server tour**: bind a TCP listener and reply with each of these stress shapes, one per session minute:
  - `HTTP/1.1 200 OK\r\n\r\n{}` (no Content-Length, no Connection header — the historic issue #2 trigger; but now in the new code path).
  - `HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n{...truncated body...}` (claimed length > delivered).
  - `HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n` (empty body — current code calls `find('{')` which fails ⇒ "no JSON" error; verify it's caught, not panic).
  - `HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{"x":\r\n0\r\n\r\n` (chunked — code does NOT understand chunked).
  - `HTTP/1.1 500 Internal Server Error\r\n\r\nupstream timeout` (5xx with non-JSON body).
  - Slowloris: send headers + open brace, then 1 byte/sec for 60 s. Verify the 5 s read timeout actually fires.
  - 0-byte close immediately (peer reset on accept).
  - 256 KiB body (matches the buffer cap at line 121).
  - 256 KiB + 1 byte body — does the buffer cap silently truncate, returning malformed JSON?
- **Magic-number tour** (UDP path): send packets with `magic = 0xC5110002` (off-by-one ID), `magic = 0xC5110003` but `n < FEATURE_PKT_SIZE`, `magic = 0xC5110003 && n = exactly FEATURE_PKT_SIZE` (boundary), `magic with NaN/Inf in feature bytes` (assert clamping kicks in).

**Stop when:**
- All 9 hostile-server shapes have been probed and the result documented.
- All 5 magic-number variants have been probed.
- OR a defect is found that requires an Eng fix before continuing.

**Risks:**
- `read_to_end`-style hang under keep-alive (the historic shape — should be fixed but unverified).
- 256 KiB cap silently truncates valid responses, leading to a "no JSON" error that operators read as "API broken" when it's "response too big".
- Chunked encoding is silently accepted as garbage.
- f32 NaN escapes the clamp (e.g. via `f64::from(f32::NAN).clamp(-1.0, 1.0)` → NaN passes through).

**Debrief deliverable:** session report listing each probe shape, the observed behaviour, and any defect filed. Each defect needs a hermetic `#[test]` in `crates/cog-sensor-sources/tests/`.

---

## CHARTER 2 — Presence-gating audit across the four "vital-sign" cogs

**Mission:** Confirm whether issue #3 (alerts fire while `presence_detected=false`) exists in `respiratory-distress`, `cardiac-arrhythmia`, `vital-trend`, `seizure-detect` — beyond `health-monitor` where it's confirmed.

**Areas:** F (Function), specifically the alert-emission branch.

**Time:** 60 min.

**Setup:**
- Open the four cog `main.rs` files side-by-side: `src/cogs/{respiratory-distress,cardiac-arrhythmia,vital-trend,seizure-detect}/src/main.rs`.
- Have the issue #3 root-cause sketch open for reference (`alerts.push(...)` not gated on `is_present`).

**Tactics:**
- **Code tour**: for each cog, `grep -n "alerts.push\|is_present\|presence" src/cogs/<id>/src/main.rs`.
- For each `alerts.push` site, walk back to the enclosing `if`/branch — does it transitively check presence? Is presence even computed?
- For each cog without presence detection, that's a *finding* (silent emission is by-design, but documented?).
- Run each cog `--once` against a synthetic-data seed (or a stub) and capture JSON output. Are any alerts present when the input is pure noise?
- **Inversion test**: feed a square wave (50% duty, 1 Hz) — should produce confident `presence_detected:true`. If a cog emits `presence:false` AND alerts on this, that's the inverse bug.

**Stop when:**
- All 4 cogs have been read AND run.
- Per-cog row added to the audit table (charter debrief).

**Risks:**
- Same anti-pattern as issue #3 but in 1-3 sibling cogs; each is one more false-positive medical alert source.
- `seizure-detect` may have its own threshold logic that's *more* aggressive than `health-monitor` and worth filing separately.

**Debrief deliverable:** audit table —

```
cog                    | computes presence? | gates alerts on presence? | repro on synthetic data? | issue filed?
-----------------------|---------------------|---------------------------|---------------------------|--------------
respiratory-distress   | ?                   | ?                         | ?                         | ?
cardiac-arrhythmia     | ?                   | ?                         | ?                         | ?
vital-trend            | ?                   | ?                         | ?                         | ?
seizure-detect         | ?                   | ?                         | ?                         | ?
```

Each "no" → file a sibling issue to #3 + a regression `#[test]`.

---

## CHARTER 3 — `--source auto` race conditions on a real seed

**Mission:** Find timing edge cases in PR #6's `Auto` source-selection where the cog picks the wrong path or freezes.

**Areas:** T (Time), I (Interfaces). Files: `src/cogs/{health-monitor,ruview-densepose}/src/main.rs::fetch_batch::Auto`.

**Time:** 90 min.

**Setup:**
- Bench seed at 169.254.42.1 over USB.
- ESP32-S3 connected to the seed (or to host with the bridge script).
- Seed running cog v1.2.0 (post PR #6) of both `cog-health-monitor` and `cog-ruview-densepose`.

**Tactics:**
- **Cadence tour**: run the cog at `--interval 1`, `--interval 5`, `--interval 30`. With `AUTO_UDP_PROBE_MS = 2000`, `--interval 1` means each tick is dominated by the probe. Does the cog actually emit at 1 Hz? Confirm via journalctl timestamps.
- **Source flap**: start with bridge OFF (cog should pick `auto:seed-stream`). Mid-run, start the bridge. Does the cog flip to `auto:esp32-udp` on the next tick? Or only on restart? Document the latching behaviour.
- **Source loss**: opposite — start with bridge ON, mid-run, kill the bridge. Does the cog gracefully fall back? After how many ticks?
- **Probe collision**: start two cogs simultaneously (`cog-health-monitor` and `cog-ruview-densepose`). Both bind UDP `0.0.0.0:5006`. The second `bind` should fail with `EADDRINUSE` — does the cog handle this and fall back to seed-stream? Or does it log + continue with no UDP?
- **Probe under load**: while cog is in its 2 s probe, fire `/api/v1/sensor/stream` GETs from another shell. Do ANY of the cog's responses show staleness or skipped intervals?

**Stop when:**
- All 5 tactics exercised.
- Behavioural notes captured for each.

**Risks:**
- `EADDRINUSE` on the second cog binding — silent failure, looks like "no ESP32 data" forever.
- Source flap requires cog restart to pick up new path — operator surprise.
- 2 s probe at `--interval 1` causes effective cadence drift to >2 s/tick.

**Debrief deliverable:** session report. Any defect → a `#[test]` (unit if reproducible hermetically, `#[ignore]` integration otherwise) + an issue.

---

## CHARTER 4 — Cog crash + restart recovery (subprocess lifecycle)

**Mission:** Reproduce cog process leaks (related to seed#17 zombie reaping) on the cogs side; identify cogs that don't gracefully exit on signal.

**Areas:** O (Operations), T (Time).

**Time:** 60 min.

**Setup:**
- Bench seed.
- Install the same cog 5 times, killing the process between installs (simulating the install/uninstall churn).

**Tactics:**
- After each install/uninstall pair, `ssh seed "ps -ef | grep cog-"`. Are there zombie / orphan processes accumulating?
- Send `SIGTERM` directly to a running cog process — does it clean up its UDP socket binding? (For PR #6 cogs that bind 5006 in auto mode.)
- Send `SIGKILL` — same check.
- Run `--interval 1` for 5 minutes; observe `top` for memory growth. The cogs are simple; no growth is the expected baseline.
- For 3 random cogs from PR #7 (`adversarial`, `swarm-mesh-manager`, `intrusion-detect-ml`), confirm they actually exit after a single `--once` pass (they should — but PR #7 is a mass mechanical change, easy to break).

**Stop when:**
- 5 install/uninstall cycles performed.
- 3 cogs verified for `--once` exit semantic.

**Risks:**
- Bound UDP port leaks (no `Drop` impl on raw `UdpSocket` would matter only across process restarts, but if the cog respawns inside the same process — unlikely).
- Memory growth per tick (Hampel filter buffer not bounded?).
- A cog that hangs waiting on stdin (a debug `read_line` accidentally left in).

**Debrief deliverable:** session report. Memory ceiling observation per cog.

---

## CHARTER 5 — DSP correctness with adversarial-but-plausible input

**Mission:** Find DSP failure modes that don't crash but produce wildly wrong output — the kind of bug that erodes operator trust without ever firing a panic.

**Areas:** F (Function), D (Data). Targets: `health-monitor` (zero-crossing BPM, Welford), `ruview-densepose` (Hampel filter), `vital-trend` (vital-sign smoothing), `breathing-sync` (sync detection).

**Time:** 90 min.

**Setup:**
- Hermetic — feed inputs to each cog's DSP function via a small `#[cfg(test)]` driver.

**Tactics:**
- **Boundary inputs**:
  - Zero-length signal → `zero_crossing_bpm` returns 0 (verify, don't trust).
  - Signal of length 1 → ditto.
  - Signal where every sample is exactly 0.0 → no crossings → BPM 0 (verify).
  - Signal of `[1e-15, -1e-15, 1e-15, -1e-15, ...]` → many "crossings" but no real signal → BPM should reject as noise but doesn't.
- **Drift inputs**:
  - Sine wave at exactly the Nyquist frequency (sample_rate/2) → unstable BPM estimate.
  - DC offset (signal = constant 0.5) → 0 crossings → BPM 0 ✓.
  - DC offset + tiny noise (signal = 0.5 + N(0, 1e-6)) → BPM jitter — what does the cog emit?
- **Hampel filter** in `ruview-densepose`: feed a stream where every 3rd sample is an outlier — does the filter actually correct? `hampel_corrections` field in JSON should be > 0.
- **NaN/Inf injection**: a single `f64::NAN` in the input vector. Does the BPM estimator produce NaN output? Does the JSON serialise as `null` or panic?

**Stop when:**
- 8 boundary inputs probed across the 4 cogs.

**Risks:**
- BPM 0 when there's actually signal (false negative on presence).
- BPM ~50/100/150 quantization (the issue #3 evidence pattern — synthetic data quantizes to these values; if the cog also produces these on real low-amplitude noise, alerts fire on nothing).
- NaN propagates to output JSON, downstream consumers parse `"NaN"` as a string and fail.

**Debrief deliverable:** for each surprising output, a `proptest` skeleton in `tests/property/dsp_invariants.rs`.

---

## CHARTER 6 — Security boundary on the SaaS layer (top-level `cognitum` lib)

**Mission:** Once R4 is unblocked and `cargo test --workspace` runs, exercise the `auth`/`hipaa`/`security` flows for boundary-crossing bugs.

**Areas:** S (Structure), I (Interfaces). Files: `src/auth/`, `src/hipaa/`, `src/security/`, `src/api/rate_limit.rs`, `src/validation/`.

**Time:** 90 min.

**Setup:**
- R4 must be resolved (workspace builds).
- Postgres + Redis available locally (docker compose, or skip the gated tests).

**Tactics:**
- **JWT tour**: forge tokens with `kid` pointing at non-existent keys, expired tokens, tokens with `alg=none`, oversized claim payloads. Existing tests in `tests/unit/auth/jwt_test.rs` cover some — find the gaps.
- **RBAC tour**: every role + every endpoint matrix. `tests/unit/auth/rbac.rs` is 293 LoC; sample a few roles for "can role X reach endpoint Y" and confirm the answer matches the docs.
- **HIPAA path tour**: `tests/unit/hipaa/access_test.rs`, `session_test.rs`, `storage_test.rs` exist. Cross-check the BAA workflow (`tests/acceptance/hipaa/baa_workflow_test.rs`) against the documented HIPAA requirements — gap-fill.
- **Validation tour**: `tests/unit/validation/sql_injection_tests.rs`, `path_traversal_tests.rs` exist. Add: SSRF detection in API key prefix validation; URL canonicalisation; null-byte injection.
- **Rate limit tour**: the false-positive SAST findings (lines 296-397) are all in *test fixtures*. Audit the real production rate-limit code for: TOCTOU on the bucket counter, integer overflow on retry-after computation.

**Stop when:**
- 5 tours exercised.
- New gap-fill tests filed (added to `tests/unit/`).

**Risks:**
- Any endpoint that bypasses RBAC under specific role+path combination.
- Rate-limit bypass via `X-Forwarded-For` or similar header.
- HIPAA audit-log gap (an event that's required but not actually logged).

**Debrief deliverable:** new tests added to `tests/unit/`. Issues filed for each real gap.

---

## CHARTER 7 — Registry / cog-id parity with seed firmware

**Mission:** Find cogs whose ID, version, or schema is inconsistent across `cog.toml`, `Cargo.toml`, `src/cogs/<id>/`, and seed's shipped `registry.json` / `app-registry.json`.

**Areas:** S (Structure), D (Data). Cross-repo.

**Time:** 60 min.

**Setup:**
- Both repos checked out (`repos/cogs/` and `repos/seed/`).
- Most recent `registry.json` from `gs://cognitum-apps/registry.json` (or from seed's bundled copy).

**Tactics:**
- **Triple-key tour**: build a dataframe (small Python or jq) — for each cog dir name, capture `cog.toml [cog].id`, `cog.toml [cog].version`, `Cargo.toml [package].name`, `Cargo.toml [package].version`, `registry.json[].id`, `registry.json[].version`. Diff. Any mismatch is a finding.
- **Featured-tile parity** (cross-link to seed regression doc test 1.2): every `installCog('<id>')` in seed's `cog-store.html` must appear in cogs's `src/cogs/`.
- **Schema parity**: `cog.toml [config]` keys must match the `cog.config[]` array shipped in `registry.json`. Per seed PR #104, the modal renders from the registry array; a drift here means the modal can't be saved.

**Stop when:**
- All 90 cogs cross-checked.
- Drift table built.

**Risks:**
- Drift causes "Install fails for cog X" or "Configure modal can't save".
- The historic case (PR #5: `ruview-densepose` cog.toml=0.5.0 / Cargo.toml=1.0.0 / registry=2.0.0) reaffirms this is a real failure mode.

**Debrief deliverable:** cross-repo issue (filed in cogs, cross-linked in seed) for each drift. Hermetic meta-test in `tests/registry_version_consistency.rs`.

---

## CHARTER 8 — `cognitum-sim` simulation determinism

**Mission:** Find non-determinism in the sim crates that would make sim-driven cog tests flaky. Today: `cognitum-processor` has 268 tests; if any are flaky, that's noise that drowns out real cog-level failures.

**Areas:** T (Time), F (Function). Files: `cognitum-sim/crates/cognitum-{processor,sim,raceway,wasm-sim}/`.

**Time:** 60 min.

**Setup:**
- Run `cargo test -p cognitum-sim --release` 10 times in a row, capture pass/fail per run.
- Run `cargo test -p cognitum-processor --release -- --test-threads=1` and `--test-threads=8`. Compare results.

**Tactics:**
- **Stress repeat**: the 10-run loop. Any test that flips even once is flaky. Capture `RUST_TEST_NOCAPTURE=1` for failures.
- **Thread-count tour**: shared mutable state (e.g. raceway routing tables) is a common source of flakiness when tests run in parallel.
- **Time tour**: search for `SystemTime::now`, `Instant::now`, `thread::sleep` in test code — anything time-based is suspect.

**Stop when:**
- 10 runs complete.
- Per-test flake rate computed for any test that fails ≥ once.

**Risks:**
- Flaky processor tests dilute confidence in the whole sim.
- A perf test (`cognitum-sim/cognitum-raceway/tests/performance_tests.rs`) is timing-sensitive by definition; needs to be marked `#[ignore]` or moved to a perf bucket.

**Debrief deliverable:** flake rate per test. Pull-request to gate flaky tests behind `#[ignore]` until stabilised (the seed `qe-flaky-hunter` skill applies here).

---

## CHARTER 9 — `crates/cog-sensor-sources/.claude-flow` hygiene + `.gitignore` coverage

**Mission:** Find every machine-generated artifact in the diff history of the open PRs that should not be committed, and patch `.gitignore` to keep them out forever.

**Areas:** S (Structure). Cross-PR.

**Time:** 30 min.

**Setup:**
- `gh pr diff <n>` for each open PR.
- Search for: `.claude-flow/`, `.agentic-qe/`, `*.db`, `dist/`, `target/`, `node_modules/`, `.DS_Store`.

**Tactics:**
- For each PR, list any path under one of the above in the diff.
- Cross-check against current `.gitignore` (PR #1 + PR #6 are partial overlap).

**Stop when:**
- All 5 PRs scanned.

**Risks:**
- `.trend-cache.json` (1 line, present in PR #7) is harmless but signals the hygiene gap.
- An accidentally-committed `*.db` could be tens of MB and pollute history.

**Debrief deliverable:** `.gitignore` patch + per-PR cleanup commit list.

---

## Charter cadence + ownership

| Charter | Cadence | Default owner |
|---|---|---|
| 1 — sensor-sources hostile-server | once now (D2-D3); re-run on any PR that touches `cog-sensor-sources` | QE |
| 2 — presence-gating audit | once now (D4); add to per-PR checklist for any health-related cog | QE |
| 3 — `--source auto` race | once after PR #6 merges; re-run with bench seed monthly | QE |
| 4 — subprocess lifecycle | once now; re-run after PR #7 merges | QE shared with Eng |
| 5 — DSP correctness | once now; re-run with `proptest` in CI continuously | QE |
| 6 — SaaS security boundary | once R4 unblocks; re-run quarterly | QE shared with security |
| 7 — registry parity | once now (cross-repo); make hermetic test mandatory afterwards | QE |
| 8 — sim determinism | once now; re-run if anyone reports a flake | QE |
| 9 — repo hygiene | once now (D1); add `.gitignore` once and forget | QE |

Sessions are stored under `docs/qe/sessions/<charter-id>-YYYY-MM-DD.md` with the SBTM standard sections (charter, area, start/end time, tasks, test notes, bugs, issues, %time-on-charter).
