# Cognitum Cogs — Test Plan

**Version:** 1.0
**Date:** 2026-04-30
**Strategy doc:** [`test-strategy.md`](./test-strategy.md)

This plan operationalises the strategy: per-PR and per-area scope, owners, schedule, environments, and entry/exit gates. Risk-based — items are ordered by R1..R8 from the strategy.

---

## 1. Per-PR test scope

### PR #1 — `docs: add CLAUDE.md` (`add-claude-md`, head `4e6c819`)

**Status:** open, `mergeable: CONFLICTING`. **Bug class:** docs only.

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Verify `Build & Test` block in CLAUDE.md actually runs (it claims `cargo test --test unit_tests` and `cargo bench --bench page_index_bench`) | manual | QE | XS | `unit_tests` target compiles but blocked by R4. `cargo bench --bench page_index_bench` works. |
| Resolve conflict against current `main` (rebase) | dev | Eng | XS | `.gitignore` overlap with PR #7 likely cause. |
| Confirm `.gitignore` adds `*.db`, `.agentic-qe/`, `.claude-flow/` (matches reality of this repo) | review | QE | XS | Already in PR #1 diff. |

**Exit:** rebased + merged. No regression test (docs-only).

---

### PR #4 — `feat(health-monitor): self-contained sensor sources v1.1.0` (`feat/health-monitor-esp32-uart`)

**Status:** open, `mergeable: CONFLICTING` (likely superseded by PR #6 which lands v1.2.0 unifying default). **Bug class:** feature (new sensor source).

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Confirm PR is functionally subsumed by PR #6 | review | QE | XS | PR #6 description explicitly says "Replaces / supersedes PR #4 and PR #5". |
| Either close PR #4 or rebase + cherry-pick differential | dev | Eng | S | Decision needed. |
| If kept: regression test 4.1 `parse_source_arg_accepts_documented_forms` (see `regression-suite.md` § PR #4) | unit | QE | XS | Hermetic. |
| Cross-compile smoke for armhf with `--features esp32-uart` | build | Eng | S | Per PR description, validated on Windows COM8; need armhf confirm. |

**Exit:** either closed in favour of PR #6, OR rebased + tests added + armhf build green.

---

### PR #5 — `feat(ruview-densepose): self-contained sensor sources v1.1.0` (`feat/ruview-densepose-esp32-uart`)

**Status:** open, `mergeable: CONFLICTING`. **Bug class:** feature + drift fix (cog.toml was 0.5.0, Cargo.toml 1.0.0; PR aligns to 1.1.0).

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Same supersession check as PR #4 | review | QE | XS | PR #6 lands the unified v1.2.0. |
| **Regression test 5.1 `version_triple_consistency`** — every cog's `cog.toml.version` == `Cargo.toml.version` (and matches the registry shipped from seed) | meta-test | QE | S | Hermetic. Lives in `tests/registry_version_consistency.rs` (new). Catches the 0.5.0/1.0.0 drift that motivated PR #5. |
| Real-seed smoke: `cog-ruview-densepose --once --source esp32-uart=/dev/ttyACM0` produces `keypoints=17` JSON | integration `#[ignore]` | QE | S | Needs ESP32 wired to seed. |
| Independent regression: **issue #2** (`fetch_sensors` EAGAIN loop). PR #5 changes the fetch path; verify the new `fetch_from_seed_stream` does NOT exhibit the same `read_to_end` failure mode. | unit | QE | S | See `regression-suite.md` § issue #2. |

**Exit:** version-consistency test passes, issue #2 regression test passes, real-seed smoke green on at least one bench seed.

---

### PR #6 — `feat(cogs): unify default --source=auto, prefer ESP32 UDP :5006 (v1.2.0)` (`feat/cogs-default-udp-auto`)

**Status:** open, `mergeable: MERGEABLE`. **Bug class:** feature + behaviour change (default flag changes).

This is the only PR that's mergeable today. It's also the one the user-visible "Just Works on a fleet seed with ESP32 on WiFi" experience depends on. **Highest-priority test target.**

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| **R6.1** `auto_source_falls_back_to_seed_stream_when_udp_silent` — bind UDP locally for 100 ms, no sender, assert `Auto` returns `seed-stream` source tag | unit (in `cog-health-monitor`) | QE | S | Hermetic, uses loopback UDP. Live test of the `Auto` branch added in `main.rs` for both `cog-health-monitor` and `cog-ruview-densepose`. |
| **R6.2** `auto_source_picks_esp32_udp_when_packets_arrive` — spawn a tiny task that sends one ADR-069 MAGIC_FEATURES packet to `0.0.0.0:5006`, assert source tag is `auto:esp32-udp` and `raw_features > 0` | unit | QE | M | Hermetic. The probe window in code is 2 s; test should be able to lower it via env or const-fn. |
| **R6.3** `auto_probe_does_not_block_main_loop` — measure wall-time of the auto branch when no UDP arrives; must be ≤ `AUTO_UDP_PROBE_MS + 200ms` | unit | QE | XS | Locks in the timing contract of the new default. Without this, an "innocent" change to probe length could double `--interval 1` cog cadence. |
| **R6.4** `source_tag_in_json_output_distinguishes_paths` — assert the `source` field of HealthReport (`cog-health-monitor`) and the `source` of densepose JSON contain `auto:esp32-udp` vs `auto:seed-stream` | unit | QE | XS | Operators rely on this to see which path won (per PR #6 description). |
| **R6.5** Manual UDP_PORT change (5005→5006) doesn't break existing legacy `--source esp32-udp=BIND` — backward compat | unit | QE | XS | Hermetic; just parse the arg. |
| **R6.6** Real-seed validation of both cogs: with bridge running (`scripts/esp32-uart-to-udp-bridge.py --port COM8 --target <seed-ip>:5006`), assert source flips to `auto:esp32-udp` and `raw_features > 0`. Without bridge, source is `auto:seed-stream` and JSON still emitted | integration `#[ignore]` | QE | S | Bench seed `c5d6bbf3` per memory `project_seed_wifi_connect`. |
| **R6.7** Feature-flag matrix build: `cargo build --release` (default), `--features esp32-uart`, `--features esp32-udp`, `--features esp32-uart,esp32-udp`, `--no-default-features` — for both `cog-health-monitor` and `cog-ruview-densepose` | build | Eng | S | 8 build configurations. |

**Exit:** R6.1–R6.5 pass (hermetic), R6.6 green on at least one bench seed, R6.7 8/8 builds clean.

---

### PR #7 — `feat(cogs): all 88 cogs use shared cog-sensor-sources (ADR-091 phase 3)` (`feat/cogs-shared-sources-all`)

**Status:** open, `mergeable: CONFLICTING`. **Scope:** 175 files, +1135/-1739, mass mechanical migration of 88 cogs to a shared crate. **Highest blast-radius PR in the queue.**

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| **R7.1** Remove `crates/cog-sensor-sources/.claude-flow/.trend-cache.json` from the diff before merge — machine artifact | review | QE | XS | Per memory `feedback_git_add_explicit`. |
| **R7.2** Add `tests/` directory to `crates/cog-sensor-sources` with at minimum: `fetch_sensors_returns_samples_shape`, `fetch_from_udp_window_decodes_8_le_f32`, `fetch_from_udp_window_rejects_wrong_magic`, `fetch_from_udp_window_returns_err_on_empty_window`, `fetch_from_seed_stream_handles_keep_alive_response`, `fetch_from_seed_stream_does_not_loop_on_eagain` | unit | QE | M | See `regression-suite.md` § PR #7. The 6 tests are the minimum to catch the issue #2 class of bug in the shared crate. |
| **R7.3** Per-cog smoke matrix: `cargo build --release --bin cog-<id>` for all 88 cogs — gate on **88/88 clean** | build | Eng | M | Mechanical PR, but at this scale, even a one-character mistake in the migration script (`scripts/migrate-cog-to-shared-sources.py`) would break a subset. Run `for c in src/cogs/*/Cargo.toml; do cargo build --release --manifest-path "$c"; done` and assert exit 0 each time. |
| **R7.4** Migration-script idempotence test — running `migrate-cog-to-shared-sources.py` a second time should be a no-op (no diff). | hermetic | QE | S | Locks the script's contract. |
| **R7.5** Real-seed smoke matrix: install at least 5 representative cogs on bench seed (`adversarial`, `health-monitor`, `ruview-densepose`, `swarm-mesh-manager`, `intrusion-detect-ml`) and assert each produces output > 0 within 60 s. | integration `#[ignore]` | QE | S | Bench seed via memory `project_seed_usb_access`. |
| **R7.6** Diff each cog's pre/post `main.rs` and confirm the *only* changes are: `use cog_sensor_sources::*`, removal of the local `fetch_sensors` body, replacement of call site with `cog_sensor_sources::fetch_sensors()`. Anything else is suspicious. | review | QE | M | Sample 10 random cogs from PR #7 file list. |
| **R7.7** Resolve conflict with `main` (likely overlap with PR #4/#5/#6 on the two health/densepose cogs) | dev | Eng | M | Coordinate with whoever lands PR #6 first. |

**Exit:** R7.1 cleaned, R7.2 6 tests merged in same PR, R7.3 88/88 clean, R7.5 5/5 seed smoke green, R7.6 sampling clean, R7.7 rebased.

---

## 2. Per-area test scope (independent of any one PR)

### Area A — `cog-sensor-sources` shared crate

Driven by R3 in the strategy. Test items in R7.2 above.

### Area B — Issue #2 fix (densepose EAGAIN)

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Reproduce on `main` first — install `ruview-densepose` on bench seed, wait 30 s, GET `/api/v1/apps/ruview-densepose/logs`, assert `output:[] && errors[].contains("os error 11")`. **This is the failing test.** | integration `#[ignore]` | QE | S | Locks the bug shape. |
| Fix in `cog-sensor-sources::fetch_from_seed_stream`: switch from `read_to_end` to a Content-Length-aware reader (per issue #2 fix sketch). | dev | Eng | M | Aligns with PR #7's existing partial fix (the `read` loop with `break` on TimedOut). Verify it really handles keep-alive. |
| Hermetic test: spawn a TCP listener on a chosen loopback port, respond with `HTTP/1.1 200 OK\r\nContent-Length: 50\r\nConnection: keep-alive\r\n\r\n<50 bytes JSON>` (note keep-alive!) and assert `fetch_from_seed_stream` returns within 1 s with the correct JSON. | unit | QE | M | Lock contract. Without this test, PR #7's loop-with-break-on-EAGAIN still might EAGAIN-loop on some keep-alive responses. |
| Confirm fix on bench seed under same conditions as the issue (v0.10.11.5 era, fresh install) | integration `#[ignore]` | QE | S | |

### Area C — Issue #3 fix (presence-gating)

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Reproduce on `main` — `cog-health-monitor --once` on a synthetic-data seed (no human present), capture JSON, assert `presence_detected:false && alerts.is_empty()`. **Currently fails** per issue. | unit | QE | XS | Hermetic — feed synthetic samples into the DSP pipeline. Doesn't need a seed. |
| Fix: gate `alerts.push(TACHYPNEA/TACHYCARDIA/BRADYCARDIA/VITAL_ANOMALY)` on `is_present`; gate `APNEA` on `is_present || recently_present` | dev | Eng | XS | Per issue suggested fix. Single-file change. |
| Audit: same anti-pattern in `respiratory-distress`, `cardiac-arrhythmia`, `vital-trend`, `seizure-detect` | review | QE | M | Property test that runs against all four: random `presence_detected:false` ⇒ `alerts.is_empty()`. |
| Add `require_presence_for_alerts` to `cog.toml` `[config]` (default true) — operator override path | feature | Eng | S | |

### Area D — `cognitum-sim` workspace gap-fill

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Add at least one test to `cognitum-sim/crates/cognitum-api/` (today: 0 tests) | unit | Eng | S | Minimum: a "compiles + exports the documented set" contract test. |
| Run `cargo bench --bench simulation_bench` and capture baseline; check no regression > 10% on subsequent runs | bench | QE | XS | Already wired. |

### Area E — Top-level `cognitum` lib

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| **R4 fix** — make `sqlx::migrate!` work or feature-gate it. Options in strategy § R4. | dev | Eng | M | Highest unblocking value. |
| Once R4 fixed: run `cargo test --workspace`; capture baseline pass count + any failures. | regression | QE | S | |
| Replace AQE false-positive SAST suppressions: rename `let api_key = "sk_test_123"` → `let test_api_key = "test-key-123"` in `src/api/rate_limit.rs` (lines 296, 308, 323, 340, 360, 378, 397) and `src/storage/redis.rs` (line 527); annotate `validation/sql.rs:74` injection-pattern strings with `// nosec: validator pattern` | hygiene | Eng | XS | Cleans the security signal so real findings surface. |

### Area F — Registry / version drift

| Item | Type | Owner | Effort | Notes |
|---|---|---|---|---|
| Hermetic meta-test: every `src/cogs/<id>/cog.toml [cog].version == Cargo.toml [package].version` | meta | QE | S | See `regression-suite.md` § PR #5 / Area F. Catches the 0.5.0/1.0.0 drift class. |
| Cross-repo meta-test (lives in seed): every cog id in seed's `registry.json` has a matching `<id>/cog.toml` in cogs repo, and the version triplet matches | cross-repo | QE | M | Owner: shared with seed QE — already partially covered by seed's `registry_ui_parity.rs` proposal in `seed/docs/qe/regression-tests-roadmap.md`. |

---

## 3. Schedule

This plan is timeboxed to **the next 5 working days** from 2026-04-30. Days are nominal; actual ordering depends on Eng's PR rebase cadence.

| Day | Goal | Outputs |
|---|---|---|
| D1 (2026-04-30) | This plan + strategy + charters + regression suite + exec summary committed | 5 docs in `docs/qe/` |
| D2 | R4 fix (Eng); R7.1 cleanup; R5.1 + F.1 meta-tests merged (hermetic, no PR dep) | 2 PRs into `main` |
| D3 | PR #6 regression tests R6.1–R6.5 added; PR #6 mergeable check re-run; merge | 1 PR merged |
| D4 | Issue #2 + Issue #3 reproducers committed (failing tests). Eng fixes both. | 2 PRs into `main` |
| D5 | PR #7 (mass migration) — R7.2 6 tests merged in same PR, R7.3 88/88 build matrix run, R7.5 seed smoke. Then merge. | PR #7 merged |

After D5: PRs #1, #4, #5 either rebase-and-merge or close.

---

## 4. Environment matrix

| Env | Required for | Owner |
|---|---|---|
| macOS arm64 dev box | hermetic + property tests, AQE fleet, doc work | QE primary |
| Linux x86_64 (CI runner) | `cargo test --workspace` + cross-compile checks | future CI |
| Cross-compile to armv7-unknown-linux-gnueabihf | per-PR build smoke, especially PR #4 (`serialport` feature) | Eng |
| Bench seed `c5d6bbf3-9450-4b62-8061-f6ebd7e7f1af` (Pi Zero 2 W, ruv.net WiFi) | all `#[ignore]` real-seed tests | QE — physical access at home |
| Bench seed `cognitum-c8ab` (USB-tethered) | issue #3 reproduction (synthetic data) + manual smoke | QE |
| ESP32-S3 v0.6.1-esp32 + `esp32-uart-to-udp-bridge.py` | PR #4-#7 source=auto and esp32-udp tests | QE — set up per memory `project_seed_esp32_autonomous_deployment` |

---

## 5. Test data

Hermetic test data lives in `tests/data/` (already exists for the parent lib).

For cog tests, the inputs are:

- **Synthetic samples**: deterministic sine waves at `[0.5, 1.0, 2.0]` Hz, amplitudes `[0.1, 0.3, 0.5]`. Used for DSP unit tests (BPM estimator, presence detector). Function: `tests/data/synthetic.rs::sine(freq, amp, len)`.
- **Captured ADR-069 packets**: 5 real packets recorded from a working ESP32-S3 (hex-dumped, committed as `tests/data/adr069/*.bin`). Used to test `fetch_from_udp_window` decoder.
- **Stub agent responses**: canned `application/json` responses matching the `/api/v1/sensor/stream` contract — `{"healthy":true,"sample_count":6,"sample_rate_hz":10,"samples":[{"channel":"ch0","normalized":-0.59,"value":-0.59,...},...]}`. Used for the keep-alive regression test (Area B).

PII / fleet data: none. Test data is fully synthetic.

---

## 6. Exit / done criteria for this plan

Tracked via the per-PR + per-area exits above. The plan is "done" when:

1. PR #6 merged with R6.1–R6.7 covered.
2. PR #7 merged with R7.1–R7.7 covered.
3. PR #1 merged or closed.
4. PRs #4, #5 closed (subsumed by #6/#7) or rebase-and-merged with their tests.
5. Issues #2, #3 closed with regression tests in `main`.
6. R4 (workspace build) resolved.
7. Quality gate ≥85/100.

---

## 7. Pointers

- Strategy: [`test-strategy.md`](./test-strategy.md)
- Charters: [`exploratory-charters.md`](./exploratory-charters.md)
- Regression suite (test skeletons): [`regression-suite.md`](./regression-suite.md)
- Exec snapshot: [`quality-snapshot-2026-04-30.md`](./quality-snapshot-2026-04-30.md)
- Sister project standard: `repos/seed/docs/qe/regression-tests-roadmap.md` — the seed-side equivalent of this plan, applies the same PR-template + bug-class taxonomy.
