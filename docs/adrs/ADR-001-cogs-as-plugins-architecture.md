# ADR-001: Cogs as plugins

**Status**: Accepted
**Date**: 2026-04-29

## Context

Cognitum Seed runs on a Raspberry Pi Zero 2 W (512 MB RAM, single-core
ARMv7-A at 1 GHz). It needs to support a long tail of detection / monitoring
applications — health, security, building, retail, industrial, agriculture
— without any single binary becoming a monolith that:

1. Takes too long to build,
2. Loads code paths that are never used on a given deployment,
3. Couples unrelated detection algorithms together so one bug stalls all of
   them, or
4. Forces every change to be re-OTA'd to every appliance.

We also need third parties to publish capabilities without forking the seed
firmware — a capability marketplace, not a feature checklist.

## Decision

Each capability is a **cog**: a standalone Cargo crate under
`src/cogs/<cog-id>/` that:

1. Compiles to a single ARMv7 binary (`cog-<cog-id>-arm`) with
   `panic = abort`, `lto = true`, `strip = true`, `opt-level = "s"`.
2. Reads sensor data via the shared `cog-sensor-sources` crate (ADR-091
   contract: ESP32 UDP feature stream + seed-stream HTTP fallback).
3. Writes results back to the seed via `POST /api/v1/store/ingest` over
   loopback HTTP/1.0 (no TLS, no auth — loopback is trusted).
4. Declares its UI / config / console contract in `cog.toml` (sibling to
   `Cargo.toml`).
5. Supports `--once` and `--interval N` for both one-shot console runs and
   long-running sampling.
6. Is registered automatically by being in `src/cogs/` — the build script
   `scripts/build-all-arm.sh` loops over the directory.

Cogs **do not** statically link the seed agent. They are processes the agent
spawns, monitors, and restarts.

## Consequences

### Positive

- **Bounded blast radius.** A cog crash takes down one detector, not the
  appliance. The seed agent can restart it on a backoff.
- **Selective deployment.** A grocery-store seed runs `customer-flow`,
  `dwell-heatmap`, `queue-length`. A nursery seed runs `baby-cry`,
  `snore-monitor`, `respiratory-distress`. Same firmware, different cogs.
- **Independent versioning.** Each cog has its own `Cargo.toml` version.
  Bug-fixing `glass-break` doesn't bump the firmware.
- **Third-party capability publishing.** A vendor publishes a cog binary +
  `cog.toml` to the cog store; the seed pulls and runs it. No firmware fork.
- **Tiny binaries.** With `lto`/`strip`/`opt-level=s` and a shared
  `cog-sensor-sources` crate, individual cogs land at 200–600 KB. A
  Pi Zero 2 W can hold dozens.

### Negative

- **Process overhead.** Each cog is a process; on a single-core ARM that
  matters. Mitigated by `--interval` defaulting to 3–10 s — cogs are mostly
  asleep.
- **Schema drift risk.** Every cog re-implements its `IntrusionReport`-style
  output struct. We accept this in exchange for not coupling cogs to a
  shared schema crate that would force lockstep upgrades.
- **No cross-cog memory.** A cog can't directly read another cog's state.
  They communicate only by writing vectors into the seed's RuVector store
  and reading them back via HTTP if needed.

### Neutral

- Cogs are Rust-only in v1 — WASM cogs deferred. The trade-off was tooling
  maturity vs. sandboxing; native Rust + process isolation is enough for
  now.

## Alternatives considered

- **Monolithic firmware with feature flags.** Rejected: every new capability
  forces a firmware release; flag combinatorics explode; one panic crashes
  everything.
- **WASM modules in-process.** Rejected for v1: WASM toolchain on ARMv7 was
  immature when ADR-091 was written; sandboxing was attractive but the
  process model gave us the same isolation with less complexity.
- **Lua / JS scripting.** Rejected: cogs need real signal-processing work
  (FFTs, Welford stats, DTW). Scripting languages on a Pi Zero 2 W with
  512 MB RAM are too slow and too memory-hungry.
- **Shared schema crate (`cog-types`).** Rejected: would force every cog to
  bump in lockstep when the schema changes. We prefer schema duplication.

## Contract summary

A cog must:

```toml
# cog.toml
[cog]
id = "fall-detect"
name = "Fall Detection"
version = "1.0.0"
category = "health"
description = "..."
binary = "cog-fall-detect-arm"

[config]
# typed config knobs surfaced to the seed UI

[console]
allowed_commands = ["--once", "--once --interval 5"]
max_runtime_secs = 15
output_limit_bytes = 65536
```

```toml
# Cargo.toml
[package]
name = "cog-fall-detect"
version = "1.0.0"
edition = "2021"

[[bin]]
name = "cog-fall-detect"
path = "src/main.rs"

[dependencies]
cog-sensor-sources = { path = "../../../crates/cog-sensor-sources" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

```rust
// src/main.rs — minimal contract
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = parse_interval(&args).unwrap_or(3);

    loop {
        if let Ok(sensors) = cog_sensor_sources::fetch_sensors() {
            let report = analyze(&sensors);
            println!("{}", serde_json::to_string(&report).unwrap_or_default());
            let _ = store_to_seed(&report);
        }
        if once { break; }
        std::thread::sleep(Duration::from_secs(interval));
    }
}
```

## Optional ruvnet/ruview integration

A cog **may** opt into ruview WiFi-CSI input by:

1. Declaring a `ruview` feature in `Cargo.toml`:
   ```toml
   [features]
   default = []
   ruview = []
   ```
2. Branching on the `--ruview-mode` flag at runtime to interpret the
   ESP32 feature stream as densepose-style keypoints rather than raw
   amplitude features.
3. Documenting in its ADR which ruview features it consumes (skeleton,
   pose, motion vector) and what falls back when ruview isn't present.

This keeps non-vision cogs free of ruview dependencies while letting
vision-aware cogs gracefully add depth when CSI data is available.

## Status

Accepted as the architecture for all v1 cogs (90 cogs as of 2026-04-29).
This ADR codifies what was previously implicit in the codebase. New cog
ADRs (002+) reference this one rather than re-explaining the model.

## Related work

- ADR-091 (in seed repo): self-contained sensor sources contract for cogs.
- ADR-069 (in seed repo): ESP32 MAGIC_FEATURES UDP packet format.
- ADR-092 (in seed repo): cognitum framework / browser-MCP / Claude plugins.
- ADR-093 (in seed repo): SDK rollout and dev portal.
