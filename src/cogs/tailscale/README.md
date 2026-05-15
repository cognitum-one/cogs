# cog-tailscale — Tailscale Mesh VPN cog for Cognitum Seed

Reach a Cognitum Seed from anywhere via your private Tailscale tailnet.
Userspace-mode only (no root, no kernel routing) — see
[ADR-100](../../docs/seed/ADR-100-tailscale-cog.md) for the full design.

## Status

- **Phase**: Draft, MVP scaffold complete, not yet cross-compiled or published.
- **Target**: armhf (Pi Zero 2 W), shipping via `external/cogs/src/cogs/tailscale/` and the cog registry.
- **First runnable**: pending cross-compile + a CI job that uploads the
  bundled `tailscaled` + `tailscale` binaries as assets to
  `gs://cognitum-apps/cogs/arm/tailscale/<version>/`.

## What works in this scaffold

The Rust binary (`src/main.rs`) implements the full lifecycle:

1. Reads `<app_dir>/config.json` (auth_key, hostname, serve_agent, advertise_tags).
2. Spawns `tailscaled --tun=userspace-networking` with its own state dir.
3. Waits for the local socket, then `tailscale up`.
4. Optionally `tailscale serve --bg https+insecure://127.0.0.1:8443` to expose
   the seed's agent dashboard to the tailnet.
5. Starts an HTTP API on loopback `8044`:
   - `GET /status`, `GET /health`, `GET /peers`
   - `POST /up`, `POST /logout`
6. Cleans up tailscaled on shutdown (sentinel-file driven so the agent's
   process supervisor can request graceful exit without complicating
   signal handling).

## What's NOT done yet

- Cross-compile to armhf (needs `cargo build --release --target armv7-unknown-linux-gnueabihf`)
- Asset upload to `gs://cognitum-apps/cogs/arm/tailscale/0.1.0/`
- Registry entry in `scripts/cognitum/app-registry.json`
- Dashboard cards
- Operator-facing walkthrough at `docs/seed/tailscale-cog.md`

## Build (dev / x86 — won't actually reach a tailnet without the armhf upload)

```bash
cd cogs/tailscale
cargo build --release
# Drop tailscaled + tailscale binaries into the working dir before --once:
cp /path/to/tailscaled tailscaled
cp /path/to/tailscale  tailscale
echo '{"auth_key":"tskey-auth-…","hostname":"dev-seed","serve_agent":true,"advertise_tags":"tag:seed"}' > config.json
COG_APP_DIR=. ./target/release/cog-tailscale-arm
```

## Build (production — armhf cross-compile)

```bash
docker build -f Dockerfile.armhf -t cog-tailscale-armhf:0.1.0 .
# Drop the resulting `cog-tailscale-arm` (~700 KB) into gs://cognitum-apps/cogs/arm/
```

## Configuration

| Field | Type | Default | Notes |
|---|---|---|---|
| `auth_key` | string | (empty) | One-time Tailscale auth key. Generate at https://login.tailscale.com/admin/settings/keys. Marked `secret = true` in cog.toml — masked in dashboard. |
| `hostname` | string | (empty → seed's `/etc/hostname`) | Name in the tailnet's MagicDNS, e.g. `cognitum-fb4b.tailnet-abc.ts.net`. |
| `serve_agent` | bool | `true` | Whether to `tailscale serve` the agent's HTTPS surface to the tailnet. |
| `advertise_tags` | string | `tag:seed` | Tailscale ACL tags applied to this node. |

## API surface (proxied via `/api/v1/cogs/tailscale/*`)

| Endpoint | Description |
|---|---|
| `GET /status` | Tailscale status JSON + cog_state |
| `GET /health` | 200 only if tailscaled socket responds |
| `GET /peers` | Just the peers section of status |
| `POST /up` | Re-`tailscale up` with current config |
| `POST /logout` | Drop the device from the tailnet |

## Footprint

- `tailscaled` binary on disk: ~32 MB
- `tailscale` CLI binary on disk: ~12 MB
- `cog-tailscale-arm` binary: ~700 KB (size-optimized profile)
- Idle RAM: ~25 MB
- Active mesh (5 peers): ~35 MB

Within the Pi Zero 2 W's envelope; comparable to the sparse-llm cog.

## Why userspace-only?

Tailscale supports `--tun=userspace-networking` which runs the whole stack in
process — no `/dev/net/tun`, no `iptables`, no kernel routing changes.  This
means the cog needs no root, no `CAP_NET_ADMIN`, no sudoers entries, and can
be installed/removed like any other cog.

Cost: it doesn't transparently route the seed's other traffic — only the
endpoints you explicitly `tailscale serve` are reachable.  For the seed's
intended use case (reaching the agent dashboard, sshing in via MagicDNS),
that's exactly what we want.  Subnet-router and exit-node modes are
explicit non-goals for v1.

If/when we want full kernel routing (e.g., to bridge the USB-gadget link
into the tailnet), that's a separate v2 (ADR-101 TBD) involving image-side
`apt install tailscale` and a sudoers allowlist for `tailscaled.service`.
