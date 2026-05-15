# Cognitum Cogs

Rust workspace with 90+ self-contained WASM/Rust cog applications for Cognitum Seed.

## Project Overview

Each cog is a small, single-purpose application (edge AI, sensor fusion, swarm coordination, etc.) living under `src/cogs/`. The workspace also includes the `cognitum-sim` hardware simulator library, a Rust SDK, and supporting crates for agent VMs, vector search, and energy harvesting. This repo is a submodule of the cognitum meta-repo.

## Stack

- Language: Rust (edition 2021, MSRV 1.75)
- Build: Cargo workspace
- Test: `cargo test`, `cargo bench` (criterion)
- WASM: wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys
- Node binding: napi-rs
- Async: tokio (full features)
- Serialization: serde, serde_json
- Crypto: argon2, ed25519-dalek, aes-gcm, sha2
- HTTP: axum, tower, hyper
- Database: sqlx (Postgres), redis
- Vector search: ruvector (external git dep)

## Build & Test

```bash
# Build the workspace
cargo build

# Build in release mode
cargo build --release

# Run all tests
cargo test

# Run a specific test suite
cargo test --test unit_tests
cargo test --test integration_tests
cargo test --test acceptance_tests

# Run benchmarks (criterion)
cargo bench

# Run a specific benchmark
cargo bench --bench ruvector_bench
cargo bench --bench page_index_bench

# Check without building
cargo check

# Build WASM targets (cognitum-sim)
cd cognitum-sim && cargo build -p cognitum-wasm-sim
```

## Key Directories

| Path | Contents |
|------|----------|
| `src/cogs/` | 90 self-contained cog applications (edge AI, sensors, swarm, etc.) |
| `src/apps/` | Platform app modules (browser, docker, electron) |
| `src/` | Top-level lib (`lib.rs`), api, auth, security, storage, SDK, ruvector |
| `cognitum-sim/` | Hardware simulator workspace (14 crates: core, processor, memory, raceway, wasm-sim, etc.) |
| `crates/` | Supporting crates (agentvm, fxnn, micro-hnsw-wasm, energy-harvester, thermal-brain, SDK) |
| `tests/` | Unit, integration, acceptance, performance, stress tests |
| `benches/` | Criterion benchmarks (ruvector, page-index, api, sdk, security, simulation) |
| `benchmarks/` | Extended benchmark suite with stress tests, network bench, analysis reports |
| `examples/` | Demo code (simulator bridge, SNN router, quantization, Redis rate limiting) |
| `shared/` | Shared assets and Tailwind theme config |

## Architecture Notes

- The root `Cargo.toml` defines a single package (`cognitum`) plus binary target `cognitum-api`.
- `cognitum-sim/` is a separate Cargo workspace with 14 member crates providing the hardware simulator.
- Root depends on five sim crates: cognitum-core, cognitum-processor, cognitum-memory, cognitum-raceway, cognitum-sim.
- `crates/` holds standalone utility crates (agentvm-*, micro-hnsw-wasm, thermal-brain, fxnn, energy-harvester).
- WASM compilation targets live in `cognitum-sim/cognitum-wasm` and `crates/micro-hnsw-wasm`.
- Release profile uses `opt-level = 3`, thin LTO, single codegen unit, stripped symbols.

## Critical Rules

- NEVER commit secrets, API keys, or `.env` files.
- ALWAYS run `cargo test` after code changes and verify it passes before committing.
- ALWAYS run `cargo check` to catch compilation errors early.
- ALWAYS read a file before editing it.
- Keep cog applications self-contained; shared logic belongs in `crates/` or `src/`.
- Validate all inputs at system boundaries (API handlers, SDK entry points).
- Do not save working files or tests to the repo root.
