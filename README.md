# cognitum-one/cogs

Rust cog ecosystem — 105+ self-contained apps for Cognitum Seed.

**Part of the Cognitum platform** — migrated from [ruvnet/optimizer](https://github.com/ruvnet/optimizer) (see migration tracker [#71](https://github.com/ruvnet/optimizer/issues/71)).

This repo is a submodule of the [cognitum meta-repo](https://github.com/cognitum-one/cognitum).

## Architecture

See **[docs/adrs/](docs/adrs/)** for the cog-as-plugin architecture
(ADR-001) and per-cog design decisions (ADR-002 through ADR-016).

## Paths

- `src/cogs/` — individual cog crates (one per capability).
- `crates/cog-sensor-sources/` — shared sensor input crate (ADR-091).
- `dist/{armv7,aarch64}/` — locally-built binaries (gitignored; CI publishes to GCS).
- `docs/adrs/` — architecture decision records.
- `scripts/cog-targets.py` — resolves each cog's build arches from `cog.toml` `hardware_requirement`.
- `scripts/build-all-arm.sh` / `build-all-arm64.sh` — offline/dev cross-compile helpers (build the full catalog).

## Building & publishing (ADR-020)

Each cog declares `hardware_requirement` in `cog.toml` — the source of truth for
which device(s), and thus which arch(es), it builds for:

| device | arch | Binary suffix |
|---|---|---|
| `pi-zero-2w` | armhf (`armv7-unknown-linux-gnueabihf`) | `-arm` |
| `v0-appliance` | aarch64 (`aarch64-unknown-linux-gnu`) | `-aarch64` |

CI is the source of published binaries:

- **`.github/workflows/publish-cog.yml`** — manual dispatch, one cog → builds its
  declared arches, emits the optional-integration sidecar, and publishes
  immutable production objects after the `cogs-production` environment gate.
- **`.github/workflows/publish-cog-staging.yml`** — manual-only isolated proof
  to the staging prefix through the `cogs-staging` environment.
- **`.github/workflows/build-all-cogs.yml`** — umbrella batch over the whole
  catalog (build-only smoke check, or `publish=true` / a `cogs-v*` tag to upload).

The install catalog (`app-registry.json`) lives in **cognitum-one/seed**; bump it
by hand from a publish run's immutable URI + sha256 summary. ADR-153 publication
uses environment-scoped Workload Identity Federation and create-only access;
service-account JSON keys and mutable aliases are not accepted.

An enabled static website uses the locked `vite-production-v1` profile and is
published as a deterministic content-addressed bundle. A declared OCI website
must already be pinned by digest; these workflows do not push images.

## RuView

The 2026-04 wave of cogs adds optional [ruvnet/ruview](https://github.com/ruvnet/RuView)
WiFi-CSI integration. See [docs/adrs/RUVIEW-CAPABILITY-MATRIX.md](docs/adrs/RUVIEW-CAPABILITY-MATRIX.md)
for which cogs use ruview, in what mode (none / optional / required),
and how CSI features are interpreted.

## Related repos

See [cognitum-one/cognitum](https://github.com/cognitum-one/cognitum) for the full platform view.
