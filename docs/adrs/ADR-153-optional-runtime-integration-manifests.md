# ADR-153: Optional website, Tailscale, and web MCP release manifests

**Status**: Accepted
**Date**: 2026-07-29
**Related**: ADR-001 (cogs as plugins), ADR-018 (legacy edge Tailscale cog),
ADR-020 (build and publish), website ADR-123 (runtime reconciliation)

## Context

A Cog release may now ask the control plane to reconcile three distinct
interfaces:

1. a real browser-served website;
2. a private per-deployment Tailscale attachment; and
3. a browser-reachable MCP resource.

These are optional deployment capabilities, not implied properties of every
Cog and not authority for source code or Cog Studio to deploy infrastructure.
The release pipeline previously published only mutable binary names and did not
carry a typed interface contract. Its GitHub workflows also authenticated with
a long-lived service-account JSON key that had object-admin access.

The existing `tailscale` Cog is a different feature. It is a rootless,
userspace-mode ARM application installed on a Seed. A per-deployment Tailscale
attachment is control-plane-managed ingress for a Cog workload. Conflating the
two would silently change installed fleet behavior and secret handling.

## Decision

### 1. Source declarations are optional, strict, and default off

`cog.toml` may contain the following top-level tables:

```toml
[integrations.website]
enabled = false

[integrations.tailscale]
enabled = false

[integrations.web_mcp]
enabled = false
```

An absent integration table normalizes to `{ "enabled": false }`. A disabled
table may contain no other key. Enabled tables are independently validated;
unknown fields fail the release. No source declaration may contain a
credential, token, Secret Manager resource name, observed URL, runtime status,
callback, or arbitrary command.

The canonical source keys use snake_case. The emitted JSON uses the exact
camelCase names in `cognitum.cog.integrations.v1` and website ADR-123.
`scripts/cog_integrations.py` is the stdlib-only validator and canonical
emitter. The JSON Schemas under `schemas/cog-integrations*.schema.json` are the
portable consumer contract.

### 2. Website artifacts are non-executable declarations

An enabled website represents an actual browser-served site, not a dashboard
widget. It declares:

- either the repository-owned `vite-production-v1` build profile and a safe
  relative output directory, or a digest-pinned OCI image;
- port, base path, bounded health check, auth mode, exposure, and ingress.

The manifest never carries a build command or shell arguments. An image tag is
not an identity and is rejected without `@sha256:<digest>`.

For `static-build`, CI alone expands the locked profile: Node 22,
`npm ci --ignore-scripts`, and the repository-local Vite binary. It requires a
lockfile, then packages the declared output as a deterministic archive with
normalized ownership, mode, order, and timestamps. Symlinks, special files,
empty output, more than 10,000 files, or more than 100 MiB are rejected. OCI
artifacts are referenced by digest and are not pushed by this workflow.

Private is the default policy. Public exposure requires `ingress = "public"`
and an immutable `public_access_approval.approval_ref`. Auth mode `none` is
valid only for that explicitly approved public case.

### 3. Tailscale attachment uses a logical OAuth-client binding

An enabled attachment declares an approved tailnet, hostname, one or more
approved tags, an ephemeral node, and a bounded 300–86400 second expiry. Its
only credential field is:

```toml
credential_binding = "tailscale-oauth-client"
```

That value is a logical server binding. The control plane resolves
version-pinned OAuth client material server-side and must independently
allowlist the requested tailnet and tags. The source manifest cannot name the
secret or its project. Auth keys are not accepted for new attachment plans.

ADR-018's existing `src/cogs/tailscale` application, config, assets, and
userspace behavior remain unchanged. This ADR does not migrate or auto-enrol
that application.

### 4. Web MCP is least-authority and protocol-pinned

An enabled web MCP interface emits:

- `transport = "streamable-http"` by default;
- `protocolVersion = "2025-11-25"` always;
- an exact endpoint and bounded health check;
- OAuth 2.1 protected-resource metadata or a private tenant-token audience;
- non-empty exact tool and scope allowlists;
- exact HTTPS Origin/CORS allowlists and bounded per-minute/burst limits;
- explicit private/public exposure and ingress.

Tools naming deploy, billing, IAM, network, Tailscale, secret, approval, or
release authority are rejected. Public MCP requires OAuth and an approval
reference. Legacy SSE requires `transport = "legacy-sse"` plus
`legacy_sse_acknowledged = true`; it is never inferred.

### 5. Integration manifests are canonical and content-addressed

Every binary build emits an integration document, including all-disabled Cogs.
The exact bytes are canonical UTF-8 JSON:

- keys sorted;
- compact separators;
- one final newline;
- SHA-256 over those exact bytes.

The filename embeds that digest:

`cog-<id>-integrations-v1-sha256-<hex>.json`

The pipeline also emits a checksum sidecar. Published binary, integration, and
extra-asset objects use versioned create-only paths:

```text
gs://cognitum-apps/cogs/releases/<id>/<version>/<arch>/<kind>/sha256/<digest>/<name>
gs://cognitum-apps/staging/cogs/releases/<id>/<version>/<arch>/<kind>/sha256/<digest>/<name>
```

`kind` is `binary`, `integrations`, `website`, or `assets`. The workflows pass
`--if-generation-match=0`, so a release cannot overwrite an existing object.
Mutable legacy aliases are not written. Existing legacy objects remain
available while the Seed catalog adopts immutable URIs and digests.

### 6. GitHub publishing is WIF-only and environment-separated

No workflow accepts `credentials_json` or a service-account key.

- `.github/workflows/publish-cog-staging.yml` is manual-only, uses the
  `cogs-staging` environment, and targets only the staging prefix.
- `publish-cog.yml` and the publish job in `build-all-cogs.yml` use the
  `cogs-production` environment and separate production variables.
- Build-only batch jobs have no cloud identity.

Required environment variables:

| Environment | Provider | Service account |
|---|---|---|
| staging | `GCP_COGS_STAGING_WIF_PROVIDER` | `GCP_COGS_STAGING_PUBLISH_SA` |
| production | `GCP_COGS_PROD_WIF_PROVIDER` | `GCP_COGS_PROD_PUBLISH_SA` |

The staging WIF condition must pin:

- repository ID `1211713542`;
- owner ID `256911919`;
- event `workflow_dispatch`;
- ref `refs/heads/codex/cog-optional-web-tailscale-mcp`;
- workflow ref
  `cognitum-one/cogs/.github/workflows/publish-cog-staging.yml@refs/heads/codex/cog-optional-web-tailscale-mcp`;
- environment `cogs-staging`, when that claim is mapped.

The publisher gets only create access to its release prefix. It does not get
bucket object-admin, delete, IAM, Secret Manager, Cloud Run, or Artifact
Registry authority.

### 7. Runtime events reuse the existing control-plane outbox

Source and release manifests declare desired interfaces only. They do not
declare webhooks or invoke callbacks. Reconciliation lifecycle events use the
existing authenticated action-event/webhook/outbox pipeline owned by the
control plane. Payloads carry safe references and evidence digests, not
credentials, raw provider responses, or secret resource names.

## Required validation

A release is not deployable until all of these pass:

- all real `cog.toml` files validate, with absent integrations normalized off;
- valid fixtures cover all-disabled and all-three-enabled configurations;
- negative fixtures reject arbitrary commands, secret references, missing
  public OAuth/approval, privileged MCP tools, mutable OCI tags, missing legacy
  SSE acknowledgement, and Tailscale ingress without an attachment;
- canonical emission is byte-for-byte deterministic and its filename digest
  matches the content;
- static website archives are deterministic and reject symlinks that could
  escape the declared output;
- JSON Schemas parse and pin all three security constants;
- workflow syntax passes and no JSON-key auth remains;
- isolated GCP staging proves disabled, each interface alone, all interfaces,
  and negative authorization cases before any production environment approval.

Staging proof is evidence for release gating, not permission to mutate
production.

## Consequences

- Consumers receive one deterministic interface document for every release.
- Optional features remain independent and fail closed.
- A compromised source manifest cannot choose a shell command or secret
  resource.
- Create-only publishing removes overwrite/delete authority from the normal
  release identity.
- Seed catalog and control-plane consumers must learn the new immutable URI
  before new releases stop relying on legacy aliases.
- Re-publishing identical bytes fails closed if the object already exists;
  operators can verify the existing digest rather than overwrite it.

## Alternatives considered

- **Inline commands in `cog.toml`** — rejected because a manifest would become
  a remote-code-execution surface.
- **Tailscale auth keys or Secret Manager refs in source** — rejected because
  source authors must not select credential material or its project.
- **One combined “network enabled” flag** — rejected because website,
  Tailscale, and MCP have different auth, exposure, and lifecycle boundaries.
- **Continue mutable GCS aliases with object-admin** — rejected because
  ordinary publishing does not need overwrite or delete authority.
- **Treat ADR-018 Tailscale as the attachment implementation** — rejected
  because it changes a fleet-installed edge Cog into a cloud reconciliation
  primitive with incompatible credentials and ownership.

## Later decision: ADR-154

ADR-154 adds a second, independent stage-only release-record signature. The
Sigstore keyless bundle defined by the build-evidence pipeline remains the
artifact/transparency proof. Cloud KMS `EC_SIGN_ED25519` signs the exact
canonical website release record through the same claim-restricted staging
WIF identity, emits a public-only trust-registry artifact, and grants no new
deployment or integration reconciliation authority. Production signing
remains out of scope until the staging release and website-ingestion evidence
is reviewed.

## Later decision: ADR-155

ADR-155 closes the remaining substitution gap by placing the exact normalized
manifest, its canonical-byte digest, and the conditionally required static
website bundle digest inside operator policy and the Ed25519-signed release.
Staging passes exact emitter outputs into the builder. Production and batch
publication fail closed until a separately ratified production signing
authority exists.
