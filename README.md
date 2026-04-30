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
- `dist/arm/` — pre-compiled armhf binaries for the seed.
- `docs/adrs/` — architecture decision records.
- `scripts/build-all-arm.sh` — Docker-based armhf cross-compile.

## RuView

The 2026-04 wave of cogs adds optional [ruvnet/ruview](https://github.com/ruvnet/RuView)
WiFi-CSI integration. See [docs/adrs/RUVIEW-CAPABILITY-MATRIX.md](docs/adrs/RUVIEW-CAPABILITY-MATRIX.md)
for which cogs use ruview, in what mode (none / optional / required),
and how CSI features are interpreted.

## Related repos

See [cognitum-one/cognitum](https://github.com/cognitum-one/cognitum) for the full platform view.
