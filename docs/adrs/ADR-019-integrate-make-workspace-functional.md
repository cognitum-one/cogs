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
