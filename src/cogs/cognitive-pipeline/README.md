# cog-cognitive-pipeline

FastGRNN anomaly gate + SmolLM2 sparse-LLM inference, packaged as a Cognitum Cog.

**Status:** scaffold — agent-side proxy infrastructure (ADR-095) lands first, then PR #133's sparse-LLM modules move in.

## What this cog does

1. Consumes sensor input (IMU windows by default, configurable).
2. Runs a FastGRNN gate to score each window for anomaly.
3. When the score crosses `gate_threshold`, fires a SmolLM2 (or Qwen2.5) inference to produce a natural-language summary.
4. Persists each (score, summary, RSS, latency, timestamp) tuple to a cognitive-event ring buffer.
5. Exposes the ring + on-demand generation via HTTP — `/api/v1/cogs/cognitive-pipeline/*` (agent-proxied) and registered MCP tools (`seed.cog.cognitive-pipeline.*`).

The pipeline was prototyped in [seed#133](https://github.com/cognitum-one/seed/pull/133) as an in-agent feature-gated module; it's being repackaged as a cog per [ADR-095](../../../../seed/docs/seed/ADR-095-cogs-as-api-providers.md) (cogs as API providers) and [ADR-094 (revised)](../../../../seed/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md).

## Hardware envelope

| Device | Status |
|---|---|
| Pi Zero 2 W (Cortex-A53, 512 MB) | supported — canonical target per ADR-094 |
| v0-appliance (Pi 5, Cortex-A76, 8 GB) | supported — bigger model option viable |
| Other ARM | gated; operator can `force_hardware: true` per ADR-095 |

Memory footprint at runtime:
- FastGRNN: ~10 MB
- SmolLM2-135M (Q4) loaded: ~180 MB peak
- Cognitive event ring (default cap 100): ~50 KB
- Total declared budget in `cog.toml`: 200 MB (counts against the per-seed RAM envelope)

## Configuration

See `cog.toml [config.*]`. Key knobs:

| Key | Default | Purpose |
|---|---|---|
| `model_id` | `smollm2-135m` | Which GGUF to load. `qwen2.5-0.5b-q4` is the larger option. |
| `bind_port` | `8033` | Loopback port the cog binds; agent proxies. |
| `deadline_secs` | `90` | Per-/generate wall-clock cap. |
| `gate_threshold` | `1.0` | FastGRNN score above which to trigger inference. |
| `event_ring_cap` | `100` | Cognitive-event ring buffer size; older entries evict. |

## API

All endpoints are reached via the agent's proxy at `/api/v1/cogs/cognitive-pipeline/<path>`. Direct loopback access is gated by the per-cog bearer token (ADR-095 §3).

| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/info` | open | Status, model, memory, uptime |
| GET | `/models` | open | List available models, ready state, sha |
| POST | `/generate` | paired | Run inference (OpenAI-style request body) |
| POST | `/oai_chat` | paired | OpenAI chat-completions compatible endpoint |
| GET | `/pipeline/events?since=<ts>&limit=<n>` | paired | Read cognitive event ring (delta-sync) |
| DELETE | `/pipeline/events` | paired | Flush ring |

MCP tool registrations: see `[mcp]` section of `cog.toml`. Each tool maps 1:1 to one of the HTTP endpoints above and is added to the seed's `/mcp` catalog at cog `/start`, removed at `/stop`.

## Build

```bash
# From repos/cogs root
cd src/cogs/cognitive-pipeline

# Native build (development)
cargo build --release

# Cross-compile for armv7 (Pi Zero 2 W)
docker build -f Dockerfile -t cog-cognitive-pipeline:armhf .
docker create --name cv cog-cognitive-pipeline:armhf /bin/true
docker cp cv:/cog-cognitive-pipeline ./cog-cognitive-pipeline-arm
docker rm cv
```

## Deploy

Once the agent's ADR-095 infrastructure ships, deploy via the standard cog flow:

```bash
gsutil cp cog-cognitive-pipeline-arm gs://cognitum-apps/cogs/arm/cog-cognitive-pipeline-arm
# Asset upload (one or both, depending on which model_id options the cog should support):
gsutil cp smollm2-135m-q4_k_m.gguf gs://cognitum-apps/cogs/arm/models/
gsutil cp qwen2.5-0.5b-q4_k_m.gguf gs://cognitum-apps/cogs/arm/models/
# Update app-registry.json with binary sha + asset shas
# Then on the seed: POST /api/v1/apps/install -d {"id":"cognitive-pipeline","config":{"model_id":"smollm2-135m"}}
```

## Standalone testing

The cog also runs standalone for development:

```bash
COGNITUM_COG_TOKEN=$(openssl rand -hex 32) \
  ./target/release/cog-cognitive-pipeline \
  --port 8033 \
  --model smollm2-135m

# Direct call (with the same token):
curl -H "Authorization: Bearer $COGNITUM_COG_TOKEN" http://127.0.0.1:8033/info
```

## Related

- [ADR-094 (Pi Zero Sparse LLM COG)](../../../../seed/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md) — hardware envelope, sparse-attention profile constants
- [ADR-095 (Cogs as API Providers)](../../../../seed/docs/seed/ADR-095-cogs-as-api-providers.md) — proxy + MCP + bearer + assets
- [seed#133](https://github.com/cognitum-one/seed/pull/133) — in-agent reference implementation
- [seed#132](https://github.com/cognitum-one/seed/pull/132) — showcase scripts (will need URL update post-cog)
