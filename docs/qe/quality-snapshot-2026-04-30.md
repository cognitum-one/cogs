# Cognitum Cogs — Quality Snapshot

**Date:** 2026-04-30
**Audience:** Ruv + senior eng
**Cadence:** one-off (intent: become weekly once per-PR test discipline lands)
**Reference docs:** [`test-strategy.md`](./test-strategy.md), [`test-plan.md`](./test-plan.md), [`exploratory-charters.md`](./exploratory-charters.md), [`regression-suite.md`](./regression-suite.md)

---

## Bottom line

Cogs repo is **shippable per-cog but not as a workspace**. Per-cog builds are clean and individual cogs deploy fine; the workspace `cargo check` fails on a missing `migrations/` dir. **0 of 90 cogs have any tests** (~24 KLOC unit-tested at zero). The shared `cog-sensor-sources` crate that PR #7 mass-migrates 88 cogs onto also has zero tests. The two open issues (#2 densepose EAGAIN, #3 health-monitor presence-gating) would have been caught by 2 hermetic tests totalling ~30 LoC. **Quality gate: 71/100 (FAIL).**

Five PRs open. Only PR #6 is mergeable today; PRs #1, #4, #5, #7 conflict with main.

## Per-PR verdict

| PR | Title | Mergeable? | Verdict | Blocker |
|----|-------|-----------|---------|---------|
| #1 | docs: add CLAUDE.md | CONFLICTING | **rebase + merge** — docs only | rebase |
| #4 | health-monitor v1.1.0 ESP32 sources | CONFLICTING | **close — subsumed by #6** | decision needed |
| #5 | ruview-densepose v1.1.0 ESP32 sources | CONFLICTING | **close — subsumed by #6 + #7**; cherry-pick the version-drift fix into a meta-test | decision needed |
| #6 | All cogs default `--source=auto` v1.2.0 | **MERGEABLE** | **merge after adding regression suite tests R6.1–R6.5** | tests not in PR yet |
| #7 | 88 cogs share `cog-sensor-sources` | CONFLICTING | **block until shared-crate tests land** + remove `.claude-flow/.trend-cache.json` from diff | tests + cleanup |

## Per-issue verdict

| Issue | Title | Severity | Status | Required action |
|-------|-------|----------|--------|----------------|
| #2 | ruview-densepose `fetch_sensors` EAGAIN — 0 output for 14h | major | OPEN since 2026-04-20 | refactor `fetch_from_seed_stream` to parse Content-Length; ship hermetic regression test 2.1 (keep-alive + timeout) |
| #3 | health-monitor alerts fire while presence:false | minor severity / **high trust impact** | OPEN since 2026-04-24 | gate alerts on `is_present`; ship test 3.1; audit 3 sibling cogs (charter 2) |

## Top 5 risks (from strategy, ordered)

1. **R1 — Silent zero-output cogs.** ~24 KLOC of cog DSP with zero tests. Issue #2 already happened; will happen again in another cog. Mitigated by R3.5 real-seed smoke matrix (test 7.3).
2. **R2 — False medical alerts.** Issue #3 confirmed for `health-monitor`; same anti-pattern likely lives in `respiratory-distress`, `cardiac-arrhythmia`, `vital-trend`, `seizure-detect`. Mitigated by charter 2 audit + tests 3.1, 3.2.
3. **R3 — Untested shared crate.** PR #7 makes 88 cogs depend on `cog-sensor-sources` which has zero tests. Mitigated by tests 2.1, 2.2, 6.2 living in the shared crate before PR #7 merges.
4. **R4 — Workspace doesn't build.** `src/storage/postgres.rs:64` `sqlx::migrate!("./migrations")` against missing dir. Blocks `cargo test --workspace` and parent-lib CI. Three remediation options in strategy § R4.
5. **R5 — Registry / cog-id drift.** PR #5 motivated by `ruview-densepose` cog.toml=0.5.0 / Cargo.toml=1.0.0 / registry=2.0.0 inconsistency. Mitigated by hermetic meta-test V.1.

## What I built today (2026-04-30)

- Pulled all branches from `cognitum-one/cogs`. 5 remote branches confirmed: `main`, `add-claude-md`, `feat/health-monitor-esp32-uart`, `feat/ruview-densepose-esp32-uart`, `feat/cogs-default-udp-auto`, `feat/cogs-shared-sources-all`. No tags.
- Surveyed all 5 open PRs, both open issues, the 90-cog source tree (23,960 LoC), the supporting `crates/`, the `cognitum-sim` workspace (14 sub-crates, ~700 tests), and the parent SaaS lib.
- Initialized AQE fleet `fleet-0ad22d84` (hierarchical, 7 domains). Spawned 6 lead agents.
- Ran AQE deep analysis: code_index (166 files), quality_assess (71/100 FAIL), security SAST (9 findings — all confirmed false positives), defect_predict (no defects above threshold), coverage_analyze_sublinear (270 files, results saved).
- Confirmed workspace build failure (R4) reproducible on `add-claude-md` HEAD.
- Confirmed individual cog (health-monitor) builds clean in 4.6 s with no errors.
- Wrote 4 deliverables: test strategy, test plan, exploratory charters (9 SBTM-format), regression suite (18 test skeletons).

## What needs decisions (this week)

| Decision | Owner | Recommended path |
|----------|-------|------------------|
| Close PRs #4 + #5 in favour of #6? | Ruv | YES — close, cherry-pick the cog-toml/Cargo.toml drift fix from #5 into a hermetic meta-test |
| Fix R4 with stub `migrations/`, feature flag, or split crate? | Ruv | feature flag `[features] sqlx-migrate` so per-cog builds stay light, and add stub `migrations/0001_init.sql` guarded by the feature |
| Add `cog-sensor-sources` tests inside PR #7 or follow-on PR? | Ruv | inside PR #7 — they're the regression contract |
| Real-seed CI lane (Gap C in seed regression doc)? | Ruv + Dragan | start with manual `--ignored` runs on bench seed; revisit lane in 2 weeks |

## What needs Eng work (this week)

In priority order (matches plan D2-D5):

1. **D2** — Resolve R4 (workspace build). Land 3 hermetic meta-tests (H.1 hygiene, V.1 version drift, R.1 workspace-check CI).
2. **D3** — Extract `auto_select` from `fetch_batch` as a pure function. Add tests R6.1–R6.5 to PR #6. Merge PR #6.
3. **D4** — Refactor `cog-sensor-sources::fetch_from_seed_stream` to expose `fetch_from_url`. Ship issue #2 fix + tests 2.1, 2.2. Ship issue #3 fix + tests 3.1. Run charter 2 to scope sibling-cog audit.
4. **D5** — Land 6 shared-crate tests inside PR #7. Add `scripts/build-all-cogs.sh` + GH Actions workflow. Run real-seed smoke (test 7.3) on at least one bench seed. Merge PR #7.

## Quality metrics now → target

| Metric | Today | Target (D5+1) | Target (4 weeks) |
|--------|-------|----------------|-------------------|
| Quality gate | 71 / 100 | ≥80 | ≥85 |
| Cogs with ≥1 test | 0 / 90 | ≥3 (the 3 modified by recent PRs) | ≥10 (all "tier-1" cogs per charter 2) |
| `cog-sensor-sources` line coverage | n/a (no tests) | ≥40 % | ≥80 % |
| Workspace `cargo test` clean | NO | YES | YES |
| Open major-severity issues | 1 (cogs#2) | 0 | 0 |
| Open PRs | 5 | 1-2 | ≤2 |

## What I am NOT recommending

- Re-testing things the framework already guarantees (serde, sqlx compile-time, axum routing). Only test our contracts.
- Adding tests to all 90 cogs at once. Floor: every cog the next PR touches gets tests. Ceiling: tier-1 cogs (health, presence, anomaly, swarm) get full test files within 4 weeks.
- Mutation testing today. Need line coverage first; revisit once `cog-sensor-sources` has tests.
- A new BDD/Gherkin layer. The seed standard (regression test next to module) is a closer cultural fit and the test pyramid is unit-heavy by design.

## Pointers

- AQE fleet ID this session: `fleet-0ad22d84` (hierarchical, 12 max agents, 7 domains).
- AQE artifacts: `repos/cognitum/.agentic-qe/results/` (quality, security, defects, code-index, coverage).
- Cross-repo standard: `repos/seed/docs/qe/regression-tests-roadmap.md` — same shape applied to seed firmware.
- Sister doc still pending: `docs/qe/sessions/<charter-id>-YYYY-MM-DD.md` for each charter run.
