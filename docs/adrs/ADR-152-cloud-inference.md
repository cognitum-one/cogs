# ADR-152: Cloud-inference provider cog — the seed's Tier-3 egress

**Status**: Proposed
**Date**: 2026-07-17
**Cog**: `cloud-inference`

## Context

A Pi Zero 2 W seed can answer from its on-device sparse-LLM (ADR-094, Tier ≤ 2),
but some prompts need a frontier model. seed PR #255 (ADR-090 §e) adds tier-aware
dispatch: **offline-first by default**, and only on an explicit `Min-Tier: 3` (or
a router escalation) does it reach for the cloud — via a **provider cog over
loopback**, never by the agent opening its own outbound connection.

That deliberate split is why this cog exists. Per ADR-095's provider-cog model,
the agent proxies to a cog's loopback port (`cog_proxy`, injecting the per-cog
HMAC `COGNITUM_COG_TOKEN`); the cog owns the outbound client and its
credential. Keeping `reqwest` + a `cog_` key out of `cognitum-agent` keeps the
agent's dependency footprint and its blast radius flat on a 512 MB box.

**The decisions this cog must obey are made elsewhere and are NOT restated here:**

- **seed [ADR-106](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-106-cloud-inference-provider-cog-tier3.md)** — the seed-side contract: this cog is the Tier-3 forwarding target, its credential model, publication + preference order.
- **v0-appliance [ADR-258](https://github.com/cognitum-one/v0-appliance/blob/main/docs/adr/ADR-258-device-cloud-llm-egress-and-cog-provisioning.md)** — the shared device→cloud egress decisions (endpoint, tier-overflow policy). The v0 hub is the other egress point; this cog must match it byte-for-byte.

This ADR exists to record **the cog's own** decisions, and to satisfy the
repo's "new cog needs an ADR" gate with a document that lives where the cog does.

## Decision

### 1. A thin forwarder, not an inference engine

OpenAI-compatible `POST /v1/chat/completions` on loopback `:8040`, `ram_mb = 32`
/ `cpu_pct = 20` — no model in RAM. It forwards; the plane routes. Auth is
ADR-095 `paired` (the agent's HMAC bearer), so nothing but the on-box agent can
reach it.

### 2. Request normalization — identical to the v0 hub

`build_forward_body` forces `model: "cognitum-auto"` (the fleet never pins a
vendor model — ADR-090 §4; raw vendor ids 404 at the plane), bounds `max_tokens`
(a dropped stream is still billed), disables streaming in v1, and defaults
**`fallback_policy: "best_effort"`**.

That last one matters: meta-llm's `resolveTier` clamps `cognitum-auto` to the
key's `highestHeldTier`, and on overflow its **default** `fail_fast` returns
`403 tier_scope_insufficient` while `best_effort` caps down and stamps
`cap_degraded`. ~97% of fleet owner keys are `completions:low`-only, so
inheriting the default would 403 exactly the prompts the router judged hard.
The v0 hub makes the same call in `llm::prepare_cloud_body` — a seed's answer
must not depend on whether it went standalone or through its hub.

### 3. Upstream = the completions plane (`https://api.cognitum.one`)

Default `inference_base_url` is the completions plane. Overridable — point it at
a paired v0 hub for hub-mediated mode.

*(History, so the next reader doesn't re-litigate: this defaulted to the raw
`apicompletions-…run.app` Cloud Run URL with the note "NOT api.cognitum.one —
that fronts a different service". True when written; that domain served a
storefront catalog for **any** path, so defaulting there would have handed a cog
a bogus "success". The mapping went live 2026-07-17 — `/v1/*` now routes to
`apicompletions`, a bad key returns `401 invalid_api_key` identical to the raw
URL, and unknown paths `404`. The raw URL was always a stopgap. There is no
"meta-proxy endpoint" to target either — meta-proxy is a per-user **local
desktop** proxy (its ADR-307) that holds the user's own `cog_` and forwards to
this same plane; this cog is its edge sibling, not its client.)*

### 4. Own credential, own failure mode

`COG_CLOUD_INFERENCE_KEY_FILE` (env, secret path — never a `cli_arg`, never a
registry field) points to an absolute, agent-owned `0600` credential file. The
cog re-reads it for every completion, allowing an atomically replaced
short-lived OAuth access token to take effect without a process restart.
Missing, empty, non-regular, symlinked, or group/other-accessible files fail
closed. `COG_CLOUD_INFERENCE_KEY` remains a static backward-compatible fallback
when no file path is configured. Both are distinct from ADR-095's
`COGNITUM_COG_TOKEN` proxy bearer. Absent credential → `503`, which seed#255's
dispatch already treats as a local-degrade trigger.

A cloud `2xx` whose body has no `choices` → **`502 bad_upstream`**, never passed
through as a completion. `402`/`429` + `Retry-After` propagate verbatim — the
budget boundary is the plane's to state, not ours to mask.

## Consequences

- Both fleet egress points normalize identically; changing routing/pricing policy
  is a server-side change, not a cog release.
- The cog is a second `cog_`-bearing surface on the seed. Preference order
  (ADR-106 §3) puts **hub-mediated Tier-3 first** for paired seeds precisely to
  avoid provisioning a second credential per device; this cog is the standalone
  path.
- `COG_CLOUD_INFERENCE_KEY` must never reach `output.log` — same discipline
  ADR-095 documents for `COGNITUM_COG_TOKEN`.

## Status of verification (read this before trusting the above)

Host tests pass and the crate **cross-compiles clean** to
`armv7-unknown-linux-gnueabihf` + `aarch64-unknown-linux-gnu`. That is **not** an
on-device claim: this cog has not been installed or run on a physical Pi Zero 2 W.
Publication to `gs://cognitum-apps` has not happened either (ADR-106 open
question A). Hardware validation is tracked in seed#266.
