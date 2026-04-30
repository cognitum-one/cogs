# Cognitum Cogs — Test Strategy

**Version:** 1.0
**Date:** 2026-04-30
**Owner:** QE (Dragan Spiridonov) + Ruv
**Repo:** [cognitum-one/cogs](https://github.com/cognitum-one/cogs) @ branch `add-claude-md` (HEAD `4e6c819`)
**Audience:** senior Rust + firmware engineers; assumes familiarity with the seed firmware contract and ADR-069/ADR-091.

---

## 1. Purpose

This document defines **what we test, how we test it, and what done looks like** for the cogs repository. It is the parent document for:

- [`test-plan.md`](./test-plan.md) — per-PR + per-area scope, schedule, owners
- [`exploratory-charters.md`](./exploratory-charters.md) — SBTM charters for current bugs and risk areas
- [`regression-suite.md`](./regression-suite.md) — concrete test skeletons that lock down the contracts of the open PRs and bug fixes

The strategy applies the **Heuristic Test Strategy Model (HTSM v6.3)** product factors (SFDIPOT) and the **PACT** principles for AI-assisted testing (Proactive, Autonomous, Collaborative, Targeted).

## 2. Snapshot of the system under test

| Dimension | State 2026-04-30 |
|---|---|
| Source size | 90 cog binaries (`src/cogs/*`) — **23,960 LoC**; plus 12 supporting crates (`crates/`); plus `cognitum-sim/` workspace (14 crates); plus the parent `cognitum` lib (~30 modules). |
| Workspace build | **FAILS** — root `cargo check --workspace` errors at `src/storage/postgres.rs:64` (`sqlx::migrate!("./migrations")`, dir does not exist). Per-cog build clean (`cargo check -p cog-health-monitor` → 4.6s, 0 errors). |
| Existing cog tests | **0** — `grep -r "#\[test\]" src/cogs/` returns nothing across all 90 cogs. ~24 KLOC unit-tested at zero. |
| Existing top-level tests | ~700 unit tests in `tests/` for the SaaS layer (auth, hipaa, audit, sdk, validation, ruvector). Compile blocked by the postgres migrations error. |
| `cognitum-sim` tests | ~700 `#[test]` across the 14 sim crates; hotspot is `cognitum-processor` (268). One sim crate (`cognitum-api`) is at zero. |
| `crates/` tests | `fxnn` has 11 dedicated test files; `agentvm` has integration tests; `micro-hnsw-wasm`, `thermal-brain`, `energy-harvester`, `cognitum-sdk-ruvector`, `agentvm-*` subcrates have minimal coverage. |
| Quality gate (AQE) | **FAIL — 71/100**. Avg cyclomatic complexity 10.1 (high). Maintainability 64.7. Security 85. (See `.agentic-qe/results/quality/2026-04-30T10-43-24_report.md`.) |
| Open PRs | 5 — #1, #4, #5 conflicting with main; #6 mergeable; #7 (175 files, +1135/-1739 mass migration) conflicting with main. |
| Open issues | 2 — #2 `ruview-densepose` EAGAIN (major), #3 `health-monitor` presence-gating (minor severity but high-trust impact). |
| Branches | `main` + 5 PR branches (`add-claude-md`, `feat/health-monitor-esp32-uart`, `feat/ruview-densepose-esp32-uart`, `feat/cogs-default-udp-auto`, `feat/cogs-shared-sources-all`). |

## 3. Quality risks (ordered by impact × likelihood)

| # | Risk | Impact | Likelihood | Mitigation |
|---|------|--------|------------|------------|
| **R1** | **A cog ships, runs on a fleet seed, and silently produces zero output** (issue #2 class). 23 KLOC of cog DSP with no tests means a regression in `fetch_sensors()` is invisible until a human reads logs on a single seed. | High — operator trust, "feature works in demo, fails in field" pattern. | High — already happened twice (issue #2 densepose EAGAIN; PR #4 acknowledges the synthetic-data shadow problem PR #7 is fixing). | R3.1 contract tests on the shared `cog-sensor-sources` crate; R3.5 a real-seed smoke matrix that runs `cog --once` on every cog and asserts `output > 0` within 60 s. |
| **R2** | **A cog emits clinically/operationally meaningful alerts when no signal is present** (issue #3). The presence detector is computed but the alert branch ignores it. Same anti-pattern likely lives in `respiratory-distress`, `cardiac-arrhythmia`, `vital-trend`, `seizure-detect`. | High — false-positive medical alerts erode trust and could feed downstream notification pipelines. | High — confirmed for 1 of 4 candidate cogs; pattern is plausible in 3 others. | R3.6 audit pass + presence-gate property test; explicit `#[test]` per affected cog asserting `alerts.is_empty()` on synthetic noise input. |
| **R3** | **The shared `cog-sensor-sources` crate (PR #7, +183 LoC, 88 cog dependencies) has zero tests.** The crate's `fetch_from_seed_stream` is the same `read_to_end`-style code path that caused issue #2 — fixed differently here (loop with break-on-timeout) but never asserted. | High — a defect in this crate fails 88 cogs simultaneously. | Medium — code is small and the new shape is sounder than what it replaces. | Mandatory unit tests in the regression suite (see `regression-suite.md` § PR #7) before merge: parse-shape, UDP magic check, timeout-doesn't-loop, fallback-on-empty-UDP. |
| **R4** | **Workspace will not build today.** `src/storage/postgres.rs:64` calls `sqlx::migrate!("./migrations")` against a directory that does not exist. Both `cargo check --workspace` and `cargo test --workspace` fail with E0282 + canonicalize error. | Medium — blocks all CI gating against the parent `cognitum` lib; per-cog builds still work. | Confirmed. | One of: (a) commit a stub `migrations/` dir; (b) make the macro path conditional on a feature flag; (c) move the SaaS tier into its own crate that's optional. **R3 contract tests in `cog-sensor-sources` are unblocked because that crate is independent.** |
| **R5** | **Registry / cog-id drift.** PR #4 ships `health-monitor v1.1.0`, PR #6 ships `v1.2.0`, PR #5 ships `ruview-densepose v1.1.0`, PR #7 ships v1.2.0 across all 88 cogs. The agent-side `registry.json` and `app-registry.json` are remote (in seed firmware). Drift between cog `cog.toml.version`, `Cargo.toml.version`, registry version, and what's actually on GCS is the bug from seed PR #26 (featured-tile typo) repeating. | Medium — manifests as "Install fails" or "wrong version runs after install". | High — already observed for `ruview-densepose` (cog.toml had 0.5.0, Cargo.toml had 1.0.0, PR #5 unifies to 1.1.0). | Hermetic meta-test that walks every `cog.toml` + `Cargo.toml` + registry entry and asserts version triple-consistency. Runs on `cargo test`. |
| **R6** | **PR #7 commits machine artifacts** — `crates/cog-sensor-sources/.claude-flow/.trend-cache.json` is in the diff. Same class of problem as the AQE-hooks-drop-rvf-files feedback memory: tooling artifacts leak into commits. | Low — noise, not bug. | High in this codebase culture. | `.gitignore` patterns for `.claude-flow/`, `.agentic-qe/`, `*.db`, `dist/` (PR #1 already starts this; PR #7 partially does). Pre-commit hook to reject these paths. |
| **R7** | **Sandbox / SaaS layer security findings**: AQE SAST flagged 8 "hardcoded secrets" and 1 "dangerous eval" in `src/api/rate_limit.rs`, `src/storage/redis.rs`, `src/validation/sql.rs`. **All 9 are false positives** — they are test fixtures (`let api_key = "sk_test_123"`) and SQL-injection pattern strings inside a *validator*. | Low (but SAST tool noise erodes signal). | Confirmed false positives. | Annotate fixtures with `// nosec` markers or rename to `let test_api_key`. Suppress the validator-pattern findings via tool config. |
| **R8** | **`cognitum-api` sim crate has 0 tests** (across 14 sim crates, this is the only zero). | Medium — sim API is a contract surface for downstream consumers. | Medium — undetected drift is the failure mode. | Add a contract-shape test for the API surface (no behaviour, just "compiles and exports the documented set"). |

## 4. Test scope

### 4.1 In scope

- **All 90 cogs** under `src/cogs/`. Specifically the input-selection path (`fetch_sensors` / `fetch_batch` / `cog-sensor-sources::*`) and the alert / verdict-emission branches.
- **Shared crate `cog-sensor-sources`** (PR #7) — UDP probe, JSON shape contract, timeout/error paths.
- **Top-level `cognitum` lib** — auth, hipaa, audit, sdk, validation, ruvector, security (host-side; depends on resolving R4).
- **`cognitum-sim` workspace** — extending current ~700-test surface; targeted gap-fill in `cognitum-api` (zero tests today).
- **Open PRs** — each PR gets a regression test that fails on pre-fix code.
- **Open issues** — each issue gets a regression test that fails on `main` today.

### 4.2 Out of scope (explicit)

- Re-testing things the framework already guarantees (serde correctness, axum routing, sqlx compile-time SQL validation, criterion harness). We only test our own contracts.
- WASM browser-runtime testing of `crates/micro-hnsw-wasm` (covered upstream in ruvector).
- Hardware Verilog co-simulation (`tests/verilog_cross_validation.rs` already exists; not in this expansion).
- Cross-repo seed-side tests — those live in `repos/seed/docs/qe/regression-tests-roadmap.md`.

## 5. Test pyramid

| Level | Where it lives | Hermetic? | What it tests | Trigger |
|---|---|---|---|---|
| **Unit** | `src/<module>.rs::tests`, `src/cogs/<id>/src/main.rs::tests` (none today), `crates/cog-sensor-sources/src/lib.rs::tests` | yes | DSP primitives (Welford, BPM, Hampel, Butterworth), parsing (`parse_source_arg`), classifiers (presence detector, alert gate). Pure functions. | every `cargo test` |
| **Integration (hermetic)** | `tests/<topic>.rs` in cog crates and in `cog-sensor-sources` | yes | UDP loopback, TCP loopback against a stub agent, JSON-shape contracts between cog and shared crate. No network outside loopback. | every `cargo test` |
| **Integration (real-seed)** | `tests/<topic>.rs` with `#[ignore]` | no | Cog runs on actual Pi Zero 2 W against running `cognitum-agent`; assert output produced within N seconds. Reproduces issue #2 / #3 conditions. | manual + planned `bench-seed` CI lane (see seed regression doc, Gap C). |
| **Property** | `tests/property/<topic>.rs` using `proptest` | yes | DSP estimators are bounded (BPM ∈ [0, 200]); presence-detector debounce never flips on single sample; alert generator silent when `is_present=false` for any random input. | every `cargo test` |
| **Bench / regression-bench** | `benches/`, `benchmarks/` | yes | Existing criterion benches stay green; no regression in cogs's `--once` runtime > 2× baseline. | nightly + on PR with `bench-needed` label. |
| **Exploratory** | Charters in `exploratory-charters.md` | n/a | Discovery of unknown risks. | per-charter cadence (weekly for active areas). |

## 6. Tooling

| Need | Tool | Notes |
|---|---|---|
| Run tests | `cargo test` (per-cog) and `cargo test --workspace` (once R4 fixed) | Use `--no-run` to validate compile only when iterating fast. |
| Coverage | `cargo tarpaulin --workspace --skip-clean` | Today blocked by R4. Once unblocked, target ≥40% line coverage in `crates/cog-sensor-sources` and ≥60% in modified cogs (`health-monitor`, `ruview-densepose`). |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | Currently emits 7+ warnings (unused vars, `mut` not needed, dead fields in `cognitum-sim/time.rs:14`). |
| SAST | AQE `security_scan_comprehensive` | 9 findings 2026-04-30, all false positives. Suppress via fixture rename + validator-string annotations. |
| Mutation testing | `cargo mutants` | Future — once line coverage exists. Not in scope for this iteration. |
| Browser / UI | n/a | Cogs are headless binaries; UI lives in seed firmware (`cog-store.html`). |
| Real seed | `ssh -b <host-ip> cognitum@169.254.42.1` + `curl` to seed REST API | Per memory `feedback_seed_http_api_diagnostics`: full API at `/api/v1/apps/<id>/logs`, `/api/v1/apps/install`, `/api/v1/sensor/stream`. |
| ESP32 source | `scripts/esp32-uart-to-udp-bridge.py` (in seed repo) | For testing PR #4-#7 source=auto behaviour without an ESP32 wired to the seed itself. |
| AQE fleet | `mcp__agentic-qe__*` (fleet-id `fleet-0ad22d84` initialized 2026-04-30) | 6 lead agents spawned: code-intelligence, coverage-analysis, quality-assessment, security, defect-intelligence, test-generation. |

## 7. Environments

| Env | What runs | Why it matters |
|---|---|---|
| **Dev (macOS arm64 / Linux x86_64)** | `cargo test`, AQE analysis | Hermetic and property tests live here. Sim crates and lib code. |
| **Cross-compile (armv7-unknown-linux-gnueabihf)** | `cargo build --release --target ...` per cog | Same toolchain seed firmware uses. PR #4 calls out `serialport` Linux feature flag. |
| **Dev seed (Pi Zero 2 W)** | Cogs deployed via seed `/api/v1/apps/install`; real `cognitum-agent` running | The only place where `--source auto`, ESP32 UDP, and the actual sandbox/seccomp interplay can be validated. |
| **CI (GitHub Actions Ubuntu)** | `cargo test --workspace` once R4 unblocks; AQE quality gate; rust-cross for armhf build smoke | None of the real-seed `#[ignore]` tests run here. |
| **Future: bench-seed CI lane** | Real Pi-Zero-2-W tethered to a runner; `--ignored` test pile runs against it | See seed regression doc Gap C — same workflow shape applies here. |

## 8. Entry / exit criteria

### 8.1 Per-PR entry (gate to start review)

1. PR description includes a "Bug class" line (per the seed PR template) and a regression-test linkback.
2. `cargo build --release` clean for any cog the PR modifies.
3. PR branch is rebased on `main` (resolves the conflict status on PR #1, #4, #5, #7).

### 8.2 Per-PR exit (gate to merge)

| Bug class | Required tests |
|---|---|
| Behavioural regression (handler returned wrong value) | unit `#[test]` that fails pre-fix, passes post-fix. Lives next to the code. |
| Sandbox/permission regression | meta-test that scans the source and asserts the path is in the expected set. |
| Supply-chain regression (dep, lockfile) | `tests/lockfile_policy.rs`-style hermetic checks. |
| Cross-cog regression (anything in `cog-sensor-sources`) | both: unit test in the shared crate + smoke test on at least 3 representative cogs. |
| Real-seed-only behaviour | `#[ignore]` integration test with reproduction docstring. |

### 8.3 Project exit (release-readiness for the next cog rollout)

1. `cargo test --workspace` clean (assumes R4 resolved).
2. `cog-sensor-sources` crate ≥80% line coverage (it's small).
3. Every modified cog has at least 1 unit test (the cog ports zero today; this is the floor).
4. Real-seed smoke pass: `--once` on every modified cog returns valid JSON within 60 s, on at least one bench seed (`c5d6bbf3` or `cognitum-c8ab`).
5. Issues #2 and #3 closed with regression tests linked.
6. Quality gate ≥85/100 (today 71).

## 9. Roles + responsibilities

| Role | Activity |
|---|---|
| QE (Dragan) | Owns this strategy, the test plan, charters, and regression suite. Reviews all PRs against the entry/exit criteria. Runs the AQE fleet and the real-seed smoke. |
| Engineering (Ruv) | Implements production fixes. Each fix ships with a regression test (per § 8.2). |
| AQE fleet | Continuous: defect prediction on each PR, coverage-gap detection (once data exists), quality assessment per merge. Fleet ID `fleet-0ad22d84` (this session). |
| CI | Hosts hermetic + property tests. Reports quality gate. The real-seed lane is a future capability (see Gap C in seed regression doc). |

## 10. Reporting

- **Per-PR**: AQE quality gate result + coverage delta on modified files, posted as a `gh pr comment` (per `feedback_pr_test_evidence_comment` memory).
- **Per-week**: short status note in `docs/qe/weekly-status-YYYY-MM-DD.md` capturing tests added, bugs closed, charters opened.
- **Per-release** (cog version bump on GCS): bench-seed test matrix run, summary in this repo + cross-link in seed.

## 11. Heuristics applied (HTSM SFDIPOT × cogs)

| Factor | Coverage today | Coverage target |
|---|---|---|
| **S — Structure** | Workspace structure documented in CLAUDE.md; `cargo metadata` works at per-cog level. | Workspace `cargo check` clean (R4). |
| **F — Function** | Every cog has a `--once` run-mode + a `--help` per `cog.toml`. Behaviour mostly verified manually on bench seeds. | Per-cog smoke + DSP unit tests for the modified ones. |
| **D — Data** | Cogs consume `samples[].value` floats; emit JSON. ADR-069 magic = `0xC5110003`, 8 LE-f32 features at offset 16. Schema documented in `cog.toml` `[config]`. | Hermetic JSON-shape parity test between `cog-sensor-sources` output and what each cog's DSP expects. Schema-completeness meta-test (registry parity, see seed regression doc test 5.2). |
| **I — Interfaces** | Two: HTTP loopback to agent on port 80 (`/api/v1/sensor/stream`), and UDP `0.0.0.0:5006` (ADR-069). Console stdin/stdout JSON. | Loopback stub server in `tests/` for HTTP; UDP send-self in `tests/` for ADR-069. |
| **P — Platform** | armv7 (Pi Zero 2 W), x86_64 host, macOS host. `serialport` Linux pulls libudev. | Cross-compile smoke per PR for armhf; macOS build for dev. |
| **O — Operations** | Cogs run as subprocess of `cognitum-agent`, killed/respawned on config change (api.rs:1083 in seed). | Long-run `--interval 5` for 30 s; assert no process leak (related to seed#17). |
| **T — Time** | `--interval` between samples; `--window-ms` for source probe; `auto` mode probes for 2 s. Cog DSP uses 10 Hz sample rate. | Property test: BPM estimator output bounded over time; presence detector debounce timing deterministic. |

## 12. Done

This strategy is "done" when:

1. The four sibling docs (`test-plan`, `exploratory-charters`, `regression-suite`, `quality-snapshot`) exist and are referenced from this doc.
2. PR #6 (the one that's actually mergeable) gets the regression tests from the suite added before merge.
3. Issues #2 and #3 each get a `#[test]` skeleton from the regression suite filed on a fix branch.
4. Quality gate is re-run after R4 is resolved and the false-positive SAST findings are suppressed; expected ≥85.
