# ADR-020: Cog build & publish pipeline (CI, dual-arch, hardware-gated)

**Status**: Accepted
**Date**: 2026-06-30
**Related**: ADR-001 (cogs as plugins), ADR-095 (cogs as API providers / `hardware_requirement`), ADR-018 (tailscale cog), [cognitum-one/seed ADR-100](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-100-tailscale-cog.md)

## Context

This repo owns cog **source**, but until now it had no CI **publish** lane:

- `cargo check`/`test` ran in `ci.yml`, but binaries were produced by hand-run
  scripts (`scripts/build-all-arm.sh` → armhf, `scripts/build-all-arm64.sh` →
  aarch64) and `gsutil`-uploaded manually. `dist/aarch64/` was committed;
  `dist/armv7/` was never even committed.
- The **only** publish CI that existed lived in the *wrong repo*:
  `cognitum-one/seed/.github/workflows/publish-tailscale-cog.yaml`, a
  single-cog stub whose own header admitted "the real publish CI should live in
  cognitum-one/cogs". It also kept a **diverged duplicate** of the tailscale
  cog source under `seed/cogs/tailscale/`.
- Cogs target two devices — the Pi Zero 2 W seed (armhf) and the Pi 5 v0
  appliance (aarch64) — but `hardware_requirement` (ADR-095) was populated on
  only 2 of 108 cogs, so nothing actually gated which arch a cog built for.

The whole catalog is CSI / audio / sensor-DSP cogs that run on both the edge
seed and the more powerful v0; there are no camera/NPU-only cogs today.

## Decision

### 1. `hardware_requirement` is the single source of truth for build arches

Every cog declares `hardware_requirement` in `cog.toml` (a list; a bare string
is accepted). `scripts/cog-targets.py` maps device → arch:

| device | arch | triple | GCS prefix | suffix |
|---|---|---|---|---|
| `pi-zero-2w` | armhf | `armv7-unknown-linux-gnueabihf` | `cogs/arm/` | `-arm` |
| `v0-appliance` | aarch64 | `aarch64-unknown-linux-gnu` | `cogs/arm64/` | `-aarch64` |

A missing/empty value defaults to **both**. `ci.yml`'s `manifest-validate` job
fails the build if any cog omits it or names an unknown device. All 108 cogs are
currently `["pi-zero-2w", "v0-appliance"]`; a future Hailo-only vision cog opts
out of armhf with a one-line `["v0-appliance"]`.

### 2. Two CI workflows replace the manual scripts and the seed-side stub

- **`publish-cog.yml`** — manual `workflow_dispatch`, input = one cog id. Builds
  every arch that cog declares, uploads `cog-<id>-<suffix>` to
  `gs://cognitum-apps/cogs/{arm,arm64}/`, and prints the sha256 table in the run
  summary. This is the per-cog release path.
- **`build-all-cogs.yml`** — umbrella batch over the whole catalog (matrix from
  `cog-targets.py`, `max-parallel: 20`). `workflow_dispatch` with
  `publish=false` is a build-only smoke check; `publish=true` or a `cogs-v*`
  tag uploads. Replaces the hand-run `build-all-arm*.sh`.

The local `scripts/build-all-arm*.sh` remain for offline/dev builds; CI is now
the source of published binaries.

### 3. The install catalog (`app-registry.json`) stays in cognitum-one/seed

The catalog the agent reads to install cogs lives in the seed repo. Rather than
introduce a cross-repo write token, the publish workflows emit the sha256s in
the run summary; the operator updates `seed/app-registry.json` by hand (minimal
topology). Revisit if publish cadence makes this painful.

### 4. Cogs with bundled upstream binaries use a `publish-extra.sh` hook

`publish-cog.yml` runs `src/cogs/<id>/publish-extra.sh` if present, after
building the Rust cog binary, to stage extra assets under `out/extra/`. The
`tailscale` cog uses this to download the per-arch upstream `tailscale` /
`tailscaled` binaries (version pinned in `cog.toml [upstream]`). The umbrella
batch does **not** run the hook — re-publishing upstream VPN binaries must be
deliberate, not a side effect of a routine catalog rebuild.

## Consequences

- `cognitum-one/cogs` now owns source **and** build **and** publish for every
  cog and every arch. `seed/cogs/tailscale/` and the seed-side publish stub are
  removed; `seed/docs/seed/ADR-100` is updated to point here.
- Requires the `GCP_COGNITUM_APPS_SA` repo secret (SA JSON with `objectAdmin`
  on `gs://cognitum-apps`).
- armhf binaries for the seed fleet are produced by CI for the first time
  (previously only ever hand-uploaded).
- **tailscale version**: the binaries deployed at `cogs/arm/tailscale/0.1.0/`
  (tailscaled 34971946 / tailscale 29257464 bytes) are 1.98.1 (seed ADR-100;
  sizes match its ~33/28 MB), which Tailscale has pruned from its stable channel
  — that URL now 404s, which is why the old seed stub's `1.98.1` default would
  fail. The cog is pinned to **1.98.8** (current 1.98.x stable, a safe forward
  patch). The stale `src/main.rs` `TAILSCALE_VERSION = "1.78.1"` is an unused
  placeholder; `cog.toml [upstream]` is authoritative.
