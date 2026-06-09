# ADR-019 — Integrate the migrated cog workspace and make it fully functional

- **Status:** Proposed
- **Date:** 2026-06-08
- **Authors:** ruvnet, claude-flow
- **Related:** ADR-001 (cogs-as-plugins), MIGRATION.md, ruvnet/optimizer#71

## Context

`cognitum-one/cogs` was migrated from `ruvnet/optimizer` (55 commits, branch
`migration/ruvnet-cognitum`, 2026-04-15). MIGRATION.md's stated next step —
"review and merge this branch into main, reconciling with existing content" —
was never completed: the workspace currently **does not build**.

First-pass `cargo check --workspace` (2026-06-08) fails at the foundational
`cognitum` root lib:

```
src/storage/postgres.rs:64: error canonicalizing migration directory
  .../cogs/./migrations: No such file or directory (os error 2)
```

i.e. an `sqlx::migrate!()` macro points at a `migrations/` dir that wasn't
carried over in the migration. This is a **sequential blocker** — the 107 cogs
under `src/cogs/` and the 13 support crates depend (directly or transitively) on
the root lib, so none can be validated until it compiles. Additional per-crate
breakage (stale deps, moved paths, feature drift) is expected behind it.

The goal: make the workspace **fully functional** — `cargo build --workspace`
and `cargo test --workspace` clean, CI (`ci.yml`) green — so the 107-cog
ecosystem is consumable (and installable by the V0 appliance cog registry).

## Decision

Drive the integration on branch `integrate-cogs-make-functional` via a
**concurrent multi-agent swarm workflow** (`.claude/workflows/`), structured to
respect the sequential-then-parallel shape of the work:

1. **Foundation (sequential).** One agent fixes the workspace-level blockers
   first — restore/seed the `migrations/` dir (or repoint/guard the
   `sqlx::migrate!` path), reconcile root `Cargo.toml`/`Cargo.lock`, and any
   workspace-member/feature issues — until the root `cognitum` lib + shared
   crates compile. It then runs `cargo check --workspace` and returns the
   **structured list of crates that still fail**, with error summaries. This is
   the work-list discovery; nothing downstream is real until it lands.

2. **Per-crate fixes (concurrent fan-out).** One agent per failing crate (cog or
   support crate), each fixing compile + test errors in its own crate directory,
   in parallel. Agents run in **git worktree isolation** so concurrent edits
   can't corrupt each other's working tree. Disjoint crate dirs → no cross-talk.

3. **Adversarial verify (concurrent).** Each claimed fix is re-checked
   independently (`cargo check -p <crate>` / `cargo test -p <crate>`) by a
   separate verifier before it's accepted — a fix that doesn't actually compile
   is rejected and re-queued.

4. **Workspace gate (sequential).** Final `cargo build --workspace` +
   `cargo test --workspace` + `cargo clippy`; report green vs. residual.

### Stage B — Appliance integration, validation & optimization

"Fully functional" means functional **with the V0 appliance**, not just
build-green. After Stage A, a second workflow (`integrate-cogs-appliance.js`)
drives the end-to-end on the live cluster:

5. **Cross-compile for the appliance target.** Build the cogs for
   `aarch64`/armhf (the Pi 5 cluster) via `scripts/build-all-arm.sh` (Docker) →
   `dist/`. Only cogs that passed Stage A's verify are candidates.

6. **Deploy + install via the cog supervisor.** Publish the built cogs to the
   appliance cog registry and install them through the V0 cog supervisor
   (cognitum-cog-gateway `cog_ops` — the ADR-220 lifecycle: install → configure
   → run → status → logs → remove).

7. **E2E validation on the live cluster (concurrent fan-out).** For each
   deployed cog: install → run → assert it reaches `running` and emits the
   expected output/metrics → capture SOTA metrics → remove. Mirrors the earlier
   per-cog lifecycle proofs. A cog is "functional with the appliance" only when
   it passes this live round-trip — not merely when it compiles.

8. **Optimization.** Route each cog to the right accelerator (H8 embedding /
   H10 LLM / CPU), confirm WiFi-coexistence (12 dBm cap, ADR-240) holds under
   cog load, and record per-cog throughput/p99. Park cogs needing absent
   hardware/models as documented residual.

### Guardrails

- **No fake green.** A cog is "functional" only when it compiles AND its tests
  pass (or it has an explicit, documented `#[ignore]`/skip with a reason). No
  stubbing-out of real logic to force a build.
- **Bounded.** Fan-out covers only crates that actually fail; the per-crate
  agents do not rewrite working cogs.
- **Reversible.** All work on the integration branch; merged via PR with CI
  (`ci.yml`) as the gate, never force-pushed to main.
- **Honest residual.** Any cog that can't be made functional in-scope (missing
  external dep, hardware-only, needs a model artifact) is listed explicitly with
  the reason, not silently skipped.

## Consequences

- **Pro:** the 107-cog ecosystem becomes buildable/testable and consumable by
  the appliance; the long-pending migration reconciliation is closed; CI green
  gives a durable regression floor.
- **Con/risk:** large surface; some cogs may have irreducible external/hardware
  deps and remain documented-residual; the swarm consumes significant compute.
- **Follow-up:** once green, wire the cog catalog into the V0 appliance registry
  (ADR-226 RuView cogs path) and the armhf `dist/` build.

## Notes

- Workflow script: `.claude/workflows/integrate-cogs.js` (this repo).
- Build-state baseline captured 2026-06-08: root lib fails on the `migrations/`
  dir; 107 cogs + 13 crates pending behind it.

## Stage B results (2026-06-08) — functional WITH the appliance

Ran on the live V0 cluster via the cog supervisor (ADR-220 lifecycle).

- **Cross-compile: 107/107** cogs → `armv7-unknown-linux-gnueabihf` (armhf ELF),
  **0 failures**. `scripts/build-all-arm.sh` did not exist (only referenced in
  README/ADR-001) — created it per spec (native `gcc-arm-linux-gnueabihf` +
  rustup armv7 target; unsets the host `RUSTFLAGS=-fuse-ld=mold` that breaks ARM
  cross-link). Output: `dist/armv7/`.
- **Live validation: 101/107 functional** — each a real install → configure
  (from `cog.toml`) → start → assert running + structured output/metrics → stop
  → remove round-trip on the live appliance, left clean.
- **6 honest residuals — absent runtime input, not bugs** (all install/run/stop/
  remove cleanly): `gait-analysis` (needs ESP32 UDP :5006 sensor feed, none
  live), `anomaly-attractor` / `ppe-compliance` (need live sensor/vision input),
  `tailscale` (needs external `tskey-` auth key — parks at NeedsLogin),
  `swarm-mqtt-bridge` (needs an MQTT broker/peers), `dream-stage` (needs input
  data). These light up once their real input is present.
- **Naming fix:** `cog.toml` declares `binary = "cog-<cog>-arm"` but the build
  emitted `cog-<cog>`, forcing manual `binary_url` overrides on install. Fixed
  `build-all-arm.sh` to emit the `cog.toml` `binary` name; renamed all 107 dist
  artifacts to match → installs need no override.
- **Optimization:** accelerator placement policy — LLM/generative → H10 (v0,
  cluster-3); embedding → H8 (cluster-1/2); rest → CPU. ADR-240 WiFi cap
  (12 dBm) confirmed holding under cog load on all 4 nodes; CSI capture
  unaffected.

**Outcome:** the 107-cog ecosystem is build-green, test-green (597/0), and
functional with the appliance (101/107 live; 6 input-gated). ADR-019 complete.
