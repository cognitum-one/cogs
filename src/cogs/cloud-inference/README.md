# cog-cloud-inference

Standalone **provider cog** (ADR-090 §e, ADR-095) that gives a seed a Tier-3
cloud-inference path. It exposes the OpenAI-compatible `/v1/chat/completions`
contract on its loopback socket and forwards each request to the metered
`meta-llm` gateway with its own `cog_` key, so the seed's `neural_router`
"Powerful" arm and the `agent_runtime` tool-call loop can dispatch to it exactly
like any other API cog.

This is the **standalone** deployment (ADR-090 §3): the cog talks directly to
the cloud gateway with its own scoped key. The same binary supports
hub-mediated mode by pointing `--inference-base-url` at a paired v0 hub instead.

## Contract

| Endpoint | Auth | Purpose |
|----------|------|---------|
| `GET /info` | open | id, version, endpoint, `cloud_key_configured`, uptime |
| `GET /health` | open | liveness |
| `POST /v1/chat/completions` | paired | OpenAI-compat completion (forwarded) |
| `POST /oai_chat` | paired | legacy alias |

Behaviour mirrors the v0 hub's Tier-3 handler so both egress points are
identical:

- `model` is always forced to **`cognitum-auto`** — the gateway routes (ADR-090 §4).
- **Non-streaming only** in v1 (a dropped stream is still billed); `max_tokens`
  is defaulted + capped.
- `402`/`429` + `Retry-After` are **propagated verbatim** (the budget boundary
  stays honest, not masked as 500).
- A cloud `2xx` that isn't a completion (no `choices`) fails safe as **502**
  (`bad_upstream`) — guards against a mis-pointed `inference_base_url`.
- Transport failure (offline/DNS/TLS) → **503 `degraded`** so the agent falls
  back to the local sparse-LLM.

## Config

- `--port` (default `8040`, loopback only)
- `--inference-base-url` (default the `apicompletions` Cloud Run gateway — **not**
  `api.cognitum.one`, which currently fronts a different service)
- `--timeout-secs` (default `60`)
- **`COG_CLOUD_INFERENCE_KEY`** (env, **secret**) — the `cog_` gateway bearer.
  Never a cli-arg or registry field. Absent ⇒ completions return `503` and the
  agent degrades to the on-device model.
- `COGNITUM_COG_TOKEN` (env) — the per-cog bearer the agent injects; enforced on
  `paired` endpoints.

## Build

```bash
cargo test                                                   # host
cargo build --release --target armv7-unknown-linux-gnueabihf # Pi Zero seed
cargo build --release --target aarch64-unknown-linux-gnu     # v0 appliance
```

Published via `gh workflow run publish-cog.yml -f cog=cloud-inference`.

## Not yet wired (Phase 2b)

Installing + publishing this cog is Phase 2a. Wiring the seed `neural_router`
"Powerful" arm and `agent_runtime` tool-call loop to dispatch here (instead of
only counting the route) is the follow-on, verified on a paired seed.
