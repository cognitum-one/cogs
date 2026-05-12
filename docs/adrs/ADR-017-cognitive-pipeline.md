# ADR-017: Cognitive Pipeline cog

**Status**: Proposed
**Date**: 2026-05-07
**Cog**: `cognitive-pipeline`
**Related**: [cognitum-one/seed ADR-094 (Pi Zero Sparse LLM COG)](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md), [cognitum-one/seed ADR-095 (Cogs as API Providers)](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-095-cogs-as-api-providers.md), [cognitum-one/seed#133](https://github.com/cognitum-one/seed/pull/133) (in-agent reference implementation)

## Context

The seed currently has 105+ cogs of one architectural shape — sensor-input → DSP → result-output processors. They are tiny (4–400 KB), single-purpose, and consume sensor windows via `cog-sensor-sources`.

The Cognitive Pipeline cog is structurally different:

1. It runs an inference pipeline (FastGRNN anomaly gate + SmolLM2 sparse-LLM) that needs to **expose an HTTP API** so callers can trigger generation, query the cognitive event ring, and stream OpenAI-compatible chat completions. None of the existing 105 cogs has an HTTP server.
2. It needs a **GGUF model file** (135–507 MB) alongside the binary. None of the existing cogs ships sidecar assets — they bake everything into the binary or read from `cog-sensor-sources`.
3. It only fits on hardware with the right RAM envelope (Pi Zero 2 W's 512 MB minimum, Pi 5's 8 GB comfortable). Other cogs run anywhere.

The infrastructure for cogs-as-API-providers (proxy, per-cog bearer tokens, asset distribution, hardware gate) is defined in cognitum-one/seed ADR-095. This ADR records the cog-side decisions that must align with that.

## Decision

`cognitive-pipeline` v0.1.0 ships as a binary cog with the following shape:

### HTTP surface (loopback only)

The cog binds `127.0.0.1:<bind_port>` (default 8033). The agent proxies `/api/v1/cogs/cognitive-pipeline/<endpoint>` to this loopback server, injecting a per-cog bearer token. The cog validates the token via constant-time compare and rejects with 401 if absent or wrong.

Endpoints:

| Method | Path | Auth | Purpose |
|---|---|---|---|
| GET | `/info` | open | cog status, model, memory, uptime |
| GET | `/models` | open | list available GGUFs, ready state, sha |
| POST | `/generate` | paired | inference (OpenAI-style request body) |
| POST | `/oai_chat` | paired | OpenAI chat-completions compatible |
| GET | `/pipeline/events?since=&limit=` | paired | cognitive event ring (delta-sync) |
| DELETE | `/pipeline/events` | paired | flush ring |

### MCP tool registrations

Per cognitum-one/seed ADR-092 (framework approach: seed exposes capabilities, Claude builds applications), the cog declares tools at the seed's `/mcp` endpoint:

- `seed.cog.cognitive-pipeline.info`
- `seed.cog.cognitive-pipeline.models`
- `seed.cog.cognitive-pipeline.generate`
- `seed.cog.cognitive-pipeline.events`

Each maps 1:1 to one HTTP endpoint above.

### Model selection (operator-configurable)

`cog.toml [config.model_id]`:
- `smollm2-135m` (default) — ~135 MB Q4_K_M, ~180 MB peak RAM. Fits Pi Zero 2 W comfortably.
- `qwen2.5-0.5b-q4` — ~507 MB Q4_K_M, ~600 MB peak RAM. Pi 5 only; Pi Zero borderline.

Models are downloaded from `gs://cognitum-apps/cogs/arm/models/` at install time (declared in `cog.toml [[assets]]`), sha256-verified, placed in `/var/lib/cognitum/apps/cognitive-pipeline/`. See seed ADR-095 §4.

### Data directory contract

The cog reads two paths from a single env var, `COGNITUM_COG_DATA_DIR`, defaulting to `/var/lib/cognitum/apps/cognitive-pipeline/` (the canonical sandbox path per seed ADR-095 §4):

| File | Resolved path | Source |
|---|---|---|
| GGUF model | `<COG_DATA_DIR>/<model-id>/model.gguf` | Agent install handler downloads from `gcs_path` to `cog.toml [[assets]].filename` (which includes the `<model-id>/` prefix) |
| Tokenizer | `<COG_DATA_DIR>/<model-id>/tokenizer.json` | Optional sibling — most GGUFs embed the tokenizer; the file is consulted only when present |
| Cognitive event JSONL | `<COG_DATA_DIR>/cognitive-events.jsonl` | Cog persists ring buffer here; survives restarts |

The agent should set `COGNITUM_COG_DATA_DIR` at `/start` time (alongside `COGNITUM_COG_TOKEN`). When unset, the cog falls back to the default — that path matches what the agent already creates for cog data, so the cog runs correctly even before the agent injection lands.

This contract specifically prevents the lifted code's original behaviour (writing to `/var/lib/cognitum/cognitive-events.jsonl` outside any sandbox) which would have conflicted with the agent and made the cog non-relocatable.

### Hardware gate

`cog.toml [cog].hardware_requirements = ["pi-zero-2w", "v0-appliance"]` with `hardware_override_allowed = true`. Wizard surfaces a yellow "not officially supported" badge on other devices; operator can `force_hardware: true` to install anyway. See seed ADR-095 §6.

### Resource budget

`cog.toml [resources]`: `ram_mb = 200`, `cpu_pct = 80`. The agent's resource budget check (replacing the 3-cog count cap per seed ADR-095 §5) accounts for this when the user installs additional cogs.

### Mesh permissions

`cog.toml [mesh].permissions = ["mesh.read"]`. Cognitive event delta-sync needs to enumerate peers; outbound delivery uses the agent's existing mesh tokens, not the cog's. If push-direction is added later, `mesh.send` with explicit peer whitelist.

## Pipeline behavior

1. Sensor input via `cog-sensor-sources` (default: `seed-stream`, configurable to `esp32-uart=` or `esp32-udp=`).
2. FastGRNN anomaly gate scores each window. Above `gate_threshold` (default 1.0), trigger inference.
3. SmolLM2 (or Qwen) inference with `deadline_secs` wall-clock cap (default 90 s). Truncate gracefully on deadline.
4. Append `(score, summary, rss_mb, inference_ms, timestamp_s, sensor_type)` to the cognitive event ring.
5. Ring buffer cap = `event_ring_cap` (default 100). Older entries evict.
6. Ring is persisted to `cognitive-events.jsonl` for restart recovery (matches the in-agent prototype from cognitum-one/seed#133).
7. Mesh delta-sync: peer-aware merging via `drain_events_for_sync(N)` in the seed's mesh layer.

## Status

This v0.1.0 cog ships with the full ~5,200 LOC of `sparse_*.rs` + `sparse_pipeline.rs` lifted from cognitum-one/seed#133. The lifted modules are byte-identical apart from a stripped `#![cfg(feature = "sparse-llm")]` inner attribute and a tiny `http_compat` shim that re-exports the agent's `crate::http::Request`/`Response` and `crate::api::DeviceState` so the lifted code compiles unchanged inside the cog crate. All 116 tests from the original modules pass in the cog build.

When no GGUF is present at `/var/lib/cognitum/apps/cognitive-pipeline/`, the cog returns the same handler-shape responses as the in-agent prototype with `weights_loaded: 0` — caller-visible behavior matches PR #133's stub-mode. Real inference activates the moment a model is uploaded via `PUT /model/{id}/{filename}` (or downloaded by the agent's install handler per seed ADR-095 §4).

PR #133 stays open as the in-agent reference implementation until this cog ships through `gs://cognitum-apps`. Once the cog rolls out fleet-wide, PR #133 closes with a pointer to this cog's release.

## CLI

```
cog-cognitive-pipeline [--once|--info]
                       [--port 8033]
                       [--model smollm2-135m]
                       [--deadline-secs 90]
                       [--gate-threshold 1.0]
                       [--ring-cap 100]
```

`--once` and `--info` are listed in `cog.toml [console] allowed_commands` so the seed UI's per-cog Console can call them safely.

## Output

The sparse-LLM modules are merged into this cog (see Status above); these
examples show real on-device responses, not pre-merge stubs.

`--info` (CLI, for the cog's per-seed Console UI):

```json
{
  "cog_id": "cognitive-pipeline",
  "version": "0.1.0",
  "model": "smollm2-135m"
}
```

`GET /info` (HTTP, full endpoint catalog — real-shape, ~1 KB; abbreviated here):

```json
{
  "models": ["smollm2-135m", "qwen2.5-0.5b-q4"],
  "endpoints": {
    "generate":  "POST /api/v1/llm/sparse/generate",
    "oai_chat":  "POST /v1/chat/completions",
    "upload":    "PUT  /api/v1/llm/sparse/model/{id}/{filename}",
    "...":       "see /info on a running cog for the full list"
  },
  "layers_loaded": 0,
  "weights_cached": false,
  "max_seq": 512,
  "max_tokens": 200,
  "concurrency": "serialized",
  "busy_code": "503",
  "sampling": { "temperature": {"default":1.0,"range":[0.0,5.0]}, "...": "..." }
}
```

After a model upload, `layers_loaded` becomes 30, `weights_cached` becomes true,
and `weight_mode` in the `/generate` response reads `"gguf-tied[30L+norm]"`
(measured on Pi Zero 2 W at 0.11 tok/s with SmolLM2-135M-Instruct Q4_K_M).

`GET /pipeline/events?since=0&limit=3`:

```json
{
  "events": [
    {
      "anomaly_score": 1.30,
      "inference_ms": 42.864,
      "rss_mb": 10.515625,
      "sensor_type": "imu",
      "summary": "<natural-language summary>",
      "timestamp_s": 1778159668,
      "windows_seen": 42
    }
  ],
  "next_since": 1778159669
}
```
