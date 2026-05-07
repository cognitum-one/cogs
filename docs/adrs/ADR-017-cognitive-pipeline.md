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

This v0.1.0 cog ships as a **scaffold** in this PR. Endpoints return stub responses (`weight_mode: "stub"`); the actual ~5,200 LOC of `sparse_*.rs` + `sparse_pipeline.rs` from cognitum-one/seed#133 move in here as a follow-up once the agent-side proxy + token + asset infrastructure (cognitum-one/seed feat/cog-cognitive-pipeline-adr-095 PR) lands.

PR #133 stays open as the proven reference implementation until the cog ships, then closes with a pointer to this cog's release.

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

## Output (stub today, real after sparse-LLM merge)

`/info`:

```json
{
  "cog_id": "cognitive-pipeline",
  "version": "0.1.0",
  "status": "scaffold",
  "model": "smollm2-135m",
  "deadline_secs": 90,
  "gate_threshold": 1.0,
  "ring_cap": 100,
  "uptime_secs": 18,
  "weight_mode": "stub"
}
```

`/pipeline/events?since=0&limit=3` (when sparse-LLM modules land):

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
