# cog-cognitive-pipeline

FastGRNN anomaly gate + SmolLM2 sparse-LLM inference, packaged as a Cognitum Cog.

**Status:** functional. Full sparse-LLM modules lifted from [seed#133](https://github.com/cognitum-one/seed/pull/133); real on-device inference verified end-to-end through the agent's `/api/v1/cogs/cognitive-pipeline/*` proxy. The cog ships its current API surface (HTTP catalog + streaming PUT for GGUF uploads + cognitive-event ring). Items still pending fleet rollout are called out in [Status of ADR-095 surfaces](#status-of-adr-095-surfaces) below.

## What this cog does

1. Consumes sensor input (IMU windows by default, configurable).
2. Runs a FastGRNN gate to score each window for anomaly.
3. When the score crosses `gate_threshold`, fires a SmolLM2 (or Qwen2.5) inference to produce a natural-language summary.
4. Persists each (score, summary, RSS, latency, timestamp) tuple to a cognitive-event ring buffer.
5. Exposes the ring + on-demand generation via HTTP — `/api/v1/cogs/cognitive-pipeline/*` (agent-proxied) and the MCP tool catalog declared in `cog.toml [mcp]`.

The pipeline was prototyped in [seed#133](https://github.com/cognitum-one/seed/pull/133) as an in-agent feature-gated module and repackaged into this cog per [ADR-095](../../../../seed/docs/seed/ADR-095-cogs-as-api-providers.md) (cogs as API providers) and [ADR-094 (revised)](../../../../seed/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md). The lifted modules (`sparse_*.rs` + `sparse_pipeline.rs`, ~5,200 LOC) are byte-identical to seed#133 apart from the stripped `#![cfg(feature = "sparse-llm")]` inner attribute and a tiny `http_compat` shim — see commit `d8e4b85` for the full move.

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

Measured on Pi Zero 2 W (`1c2650b4`, fw 0.22.0) with SmolLM2-135M-Instruct Q4_K_M loaded:

```
weight_mode: gguf-tied[30L+norm]    # all 30 SmolLM2 transformer layers
layers:      30
tok/s:       0.114
kv_cache:    300 hot tokens, ~460 KB RAM
```

Real outputs are short and grammatical — a 135M model isn't factually accurate, but the full pipeline (GGUF mmap, BPE tokenizer, all 30 layers, FP16 KV cache, sparse attention) runs.

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

All endpoints are reached via the agent's proxy at `/api/v1/cogs/cognitive-pipeline/<path>`. The agent's auth gate (paired-only from WiFi; USB/loopback trusted) runs before the proxy forwards. The cog also validates a per-cog bearer token if `COGNITUM_COG_TOKEN` is set in its environment; in standalone-dev mode (no token) it accepts any `Authorization` header.

| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/info` | open | Status, model, memory, uptime, sampling-parameter catalog |
| GET | `/models` | open | List available models, `ready` state, `gguf_bytes` |
| POST | `/generate` | paired | Run inference (OpenAI-style request body) |
| POST | `/v1/chat/completions` | paired | OpenAI chat-completions canonical endpoint |
| POST | `/oai_chat` | paired | Legacy alias for `/v1/chat/completions` (kept for pre-existing PR #133 callers) |
| POST | `/v1/completions` | paired | OpenAI completion-style |
| GET | `/v1/models` / `/v1/models/{id}` | open | OpenAI models surface |
| POST | `/tokenize` | paired | Tokenizer-only call |
| PUT | `/model/{id}/{filename}` | paired | **Streaming** upload of `model.gguf` / `tokenizer.json`. The agent's cog proxy + this handler both stream chunk-by-chunk — a 101 MB GGUF lands in ~7 s through `/api/v1/cogs/cognitive-pipeline/model/...` without buffering. |
| GET | `/pipeline/events?since=<ts>&limit=<n>` | paired | Read cognitive event ring (delta-sync). Query string is preserved end-to-end (both the agent proxy and the cog's path rewrite forward it intact). |
| DELETE | `/pipeline/events` | paired | Flush ring |
| GET | `/pipeline/status` | open | Pipeline counters (`frames_seen`, `windows_seen`, `events_gated`, `summaries_generated`, `rss_mb`, `initialized`) |
| GET | `/health` | open | Liveness probe |

MCP tool catalog: declared in `cog.toml [mcp]` and **logged at cog startup** (visible via `GET /api/v1/apps/cognitive-pipeline/logs`) so it's discoverable now. **Install-time registration at the seed's `/mcp` endpoint is the deferred next-layer per ADR-095 §1 second half** — until that lands, the catalog is informational only.

## Build

```bash
# From repos/cogs root
cd src/cogs/cognitive-pipeline

# Native build (development)
cargo build --release

# Cross-compile for armv7 (Pi Zero 2 W) via Docker
docker build -f Dockerfile -t cog-cognitive-pipeline-armhf:local .

# Extract the binary out of the FROM-scratch image. Use `docker save | tar`
# rather than `docker create` + `docker cp` if your daemon's default platform
# differs from linux/arm/v7 (Docker Desktop on macOS does — it tries to pull
# a host-arch variant otherwise).
docker save cog-cognitive-pipeline-armhf:local -o /tmp/cog.tar
mkdir -p /tmp/cog-extract && tar xf /tmp/cog.tar -C /tmp/cog-extract
for blob in /tmp/cog-extract/blobs/sha256/*; do
  tar tf "$blob" 2>/dev/null | grep -q cog-cognitive-pipeline && \
    tar xf "$blob" -C . && break
done
strip cog-cognitive-pipeline 2>/dev/null || true
mv cog-cognitive-pipeline cog-cognitive-pipeline-arm
```

Result: ~1.4 MB armhf ELF, stripped.

## Deploy

### Fleet path — GCS install handler (preferred once the binary is published)

```bash
gsutil -h "Cache-Control: no-cache" cp \
  cog-cognitive-pipeline-arm \
  gs://cognitum-apps/cogs/arm/cog-cognitive-pipeline-arm

# Assets — upload whichever model(s) the cog should support:
gsutil cp smollm2-135m-q4_k_m.gguf  gs://cognitum-apps/cogs/arm/models/
gsutil cp qwen2.5-0.5b-q4_k_m.gguf  gs://cognitum-apps/cogs/arm/models/

# Update gs://cognitum-apps/app-registry.json with the binary sha256 + each
# [[assets]] sha256 (replacing the bring-up `TODO-set-on-publish` values).
# Drop the `.allow-unpublished-assets` marker file in this directory — that
# disarms the cogs CI gate which currently grandfathers the TODO shas.

# On the seed (paired):
curl -XPOST http://169.254.42.1/api/v1/apps/install \
  -d '{"id":"cognitive-pipeline","config":{"model_id":"smollm2-135m"}}'
```

The agent's install handler (ADR-095 §4, implemented in cognitum-one/seed#136) fetches the binary, evaluates each asset's `required_when` against the supplied config, downloads required assets from GCS, sha256-verifies, and reports per-asset progress in the install response.

### User path — direct PUT through the cog proxy

When the GCS upload hasn't happened yet, or for a custom/fine-tuned GGUF, users can upload directly through the proxy. The agent + cog both stream the body so a 100+ MB GGUF doesn't OOM on Pi Zero:

```bash
# 1. Install the cog binary (e.g. via fleet path above, or scp into
#    /var/lib/cognitum/cog-binaries/cog-cognitive-pipeline-arm as a
#    local-cache hint to /api/v1/apps/install).
# 2. Upload model + tokenizer through the streaming PUT:
curl -XPUT --data-binary @model.gguf \
  http://169.254.42.1/api/v1/cogs/cognitive-pipeline/model/smollm2-135m/model.gguf
curl -XPUT --data-binary @tokenizer.json \
  http://169.254.42.1/api/v1/cogs/cognitive-pipeline/model/smollm2-135m/tokenizer.json
# 3. Verify:
curl http://169.254.42.1/api/v1/cogs/cognitive-pipeline/models  # ready=true
curl -XPOST http://169.254.42.1/api/v1/cogs/cognitive-pipeline/generate \
  -d '{"prompt":"hello","max_tokens":5}'
```

The PUT response carries `"transport":"streamed"` confirming the streaming path was hit and `"bytes_written":N` for verification.

## Standalone testing

The cog also runs standalone for development:

```bash
COGNITUM_COG_TOKEN=$(openssl rand -hex 32) \
  COGNITUM_COG_DATA_DIR=/tmp/cog-dev \
  ./target/release/cog-cognitive-pipeline \
  --port 8033 \
  --model smollm2-135m

# Direct call (with the same token):
curl -H "Authorization: Bearer $COGNITUM_COG_TOKEN" http://127.0.0.1:8033/info

# Or in standalone-dev mode (no token set — accepts any auth header,
# warns at startup so it's obvious):
./target/release/cog-cognitive-pipeline --port 8033 --model smollm2-135m
curl http://127.0.0.1:8033/info
```

## Status of ADR-095 surfaces

| Surface | State in this cog / agent |
|---|---|
| §1 cog HTTP proxy + dual-API (raw + MCP-registered) | ✓ raw HTTP done, **streaming** for large bodies. MCP install-side registration deferred. |
| §2 `cog.toml` schema (`[api]`, `[mcp]`, `[resources]`, `[[assets]]`, `hardware_requirements`) | ✓ this `cog.toml` ships all five sections. |
| §3 per-cog HMAC bearer token issuance | deferred. Cog validates `COGNITUM_COG_TOKEN`; agent doesn't yet generate one. Standalone-dev mode in the meantime. |
| §4 sidecar GCS asset distribution + sha256 verify + `required_when` | ✓ implemented in cognitum-one/seed#136. GCS upload of this cog's assets pending operator decision. |
| §5 per-seed resource budget (replace 3-cog count cap) | deferred. Today's `if active_cogs < 3` fallback applies. |
| §6 hardware-requirements list + override | partial. Schema in `cog.toml`; install-time enforcement deferred. |

## Related

- [ADR-094 (Pi Zero Sparse LLM COG)](../../../../seed/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md) — hardware envelope, sparse-attention profile constants
- [ADR-095 (Cogs as API Providers)](../../../../seed/docs/seed/ADR-095-cogs-as-api-providers.md) — proxy + MCP + bearer + assets (with 2026-05-11 implementation notes)
- [seed#133](https://github.com/cognitum-one/seed/pull/133) — in-agent reference implementation (stays open until this cog rolls out fleet-wide via gs://cognitum-apps)
- [seed#132](https://github.com/cognitum-one/seed/pull/132) — showcase scripts (URL prefix updates to `/api/v1/cogs/cognitive-pipeline/` post-cog)
- [seed#136](https://github.com/cognitum-one/seed/pull/136) — agent-side proxy + ADR-095 §4 install handler
