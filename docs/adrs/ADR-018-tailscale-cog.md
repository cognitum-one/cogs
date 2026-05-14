# ADR-018: Tailscale Mesh VPN cog

**Status**: Proposed
**Date**: 2026-05-13
**Cog**: `tailscale`
**Related**: [cognitum-one/seed ADR-100 (full design rationale)](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-100-tailscale-cog.md), [cognitum-one/seed ADR-095 (Cogs as API Providers)](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-095-cogs-as-api-providers.md), [cognitum-one/seed#154](https://github.com/cognitum-one/seed/pull/154)

## Context

Operators want to reach a Seed from anywhere — homes, fleets in the field, customer sites — without exposing the seed's `:8443` to the public internet, rooting the device, or wiring per-fleet VPN concentrators.

Tailscale (WireGuard mesh + coordination server) solves this generically. The question is how to ship it on the seed *without* turning it into a default capability of the gold image. Every gold-image addition has long-term obligations: security review, supply-chain auditing, version-bump cadence, breaking-change blast radius across fleets that may not even use the feature.

The 105 cogs already in this repo are all of one shape — sensor-input → DSP → result-output processors, 4–400 KB binaries, no external network needs. Tailscale's `tailscaled` is 34 MB, opens a UDP socket to the coordination server, and has its own opinions about routing. Two structural differences from the existing cogs.

## Decision

`tailscale` v0.1.0 ships as a binary cog with userspace networking.

### Userspace mode (`tailscaled --tun=userspace-networking`)

The wrapper cog runs `tailscaled` as the seed's `genesis` user with `--tun=userspace-networking`. Consequences:

- No `/dev/net/tun`, no kernel routing, no `iptables`, no `CAP_NET_ADMIN`.
- The cog runtime stays the same shape — install/uninstall is reversible, the cog is opt-in per device, and removing it leaves no kernel residue.
- Trade-off: subnet-router + exit-node modes are unavailable in userspace mode. For those, we'd want a future v2 with a privileged install path. See seed ADR-100 §6 for the v2 plan.

### Asset distribution (ADR-095 §4 conformant)

Three GCS assets per cog version:
- `gs://cognitum-apps/cogs/arm/cog-tailscale-arm` — the wrapper cog (~1 MB stripped armhf)
- `gs://cognitum-apps/cogs/arm/tailscale/0.1.0/tailscaled-armhf` (~34 MB)
- `gs://cognitum-apps/cogs/arm/tailscale/0.1.0/tailscale-armhf` (~29 MB)

`cog.toml` lists each with its real sha256 (no `TODO-` markers — the asset-sha256 publish gate enforces this). Tailscale upstream version pinned to **1.98.1** (latest stable as of 2026-05-13).

### Naming convention (CI consistency)

- `Cargo.toml` `[[bin]] name = "cog-tailscale"` — what `cargo build` outputs locally. Matches the convention enforced by `ci.yml :: manifest-validate`: every cog's `[[bin]] name` must equal `cog-<dirname>`.
- `cog.toml` `binary = "cog-tailscale-arm"` — what gets deployed to the seed (cross-compile + strip output, with `-arm` suffix). Matches the convention used by all 105 other cogs in this repo (e.g. `cog-baby-cry-arm`).

The two names differ by design: the local build artifact is renamed at publish time. See `app-registry.json` in the seed repo (companion PR #154) for how the agent's install handler resolves the GCS URL → local binary name.

### Loopback-only API

`bind_loopback_only = true` in `cog.toml [api]`. The cog binds `127.0.0.1:8044` and is reachable only through the agent's `/api/v1/cogs/tailscale/*` proxy. The agent injects a per-cog bearer token; the cog rejects unauthenticated requests.

| Method | Path | Purpose |
|---|---|---|
| GET | `/status` | tailscale status JSON + cog_state (redacts AuthURL) |
| GET | `/health` | 200 only if tailscaled socket responds |
| GET | `/peers` | peers section of status |
| POST | `/up` | re-auth with current config |
| POST | `/logout` | drop the device from the tailnet |

### Secret handling

`auth_key` in `cog.toml [config]` is marked `secret = true` per ADR-095. The agent's pairing-required input flow handles it; the cog never writes it back to disk in plaintext and `/status` redacts `AuthURL` from output.

## Status

Proposed for v0.22.5. Ships alongside cognitum-one/seed#154 (seed-side: app-registry entry, ADR-100, publish workflow) and the v0.22.5 gold image rebuild that incorporates the create-release-image.sh sanitize fixes (separate PR).

## Out of scope

- Auto-update of the bundled Tailscale version — manual bump for now
- Dashboard cards in `/framework`
- Operator walkthrough doc
- CI to auto-cross-compile + upload to GCS (current build was manual)
- Privileged-mode v2 (subnet-router, exit-node) — separate ADR when there's a use case
