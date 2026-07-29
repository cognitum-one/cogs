# ADR-156: Release trust v2 and protected evidence custody

**Status**: Proposed
**Date**: 2026-07-29
**Related**: ADR-153 (optional integrations and WIF), ADR-154 (canonical
Ed25519 releases), ADR-155 (signed runtime integrations), website ADR-128
(release authority and staging custody)

## Context

ADR-154's signed release statement is byte-compatible with the website
consumer, but its trust and storage handoffs are not. The staging publisher
emits `cognitum.cog.release-trust.v1`, which identifies only a public key,
builder, and workflow. The website authority contract requires trust v2 to
also bind the exact KMS key version, SOFTWARE protection, SPKI fingerprint,
purpose, and numeric GitHub identities.

The workflow also targets the legacy `gs://cognitum-apps` bucket. That bucket
is publicly readable and gives the legacy CI identity object-admin authority.
It has no uniform bucket-level access, versioning, or retention contract.
Content-addressed names and `if-generation-match=0` do not make a public,
administratively mutable bucket an admitted evidence store.

A signer-key revocation invalidates every release made by that key. It cannot
withdraw one release digest while preserving the immutable release record and
other valid releases. A separate, signed, digest-specific withdrawal is
required.

## Decision

### Trust registry v2

The publisher SHALL emit `cognitum.cog.release-trust.v2`. Each key entry is
strict and binds:

- `algorithm: ed25519`;
- `kmsAlgorithm: EC_SIGN_ED25519`;
- the full `kmsKeyVersion` resource;
- `protectionLevel: software`;
- SHA-256 of the public key's SPKI DER;
- one or both explicit purposes, `release` and `withdrawal`;
- exact builder identities and workflow references; and
- numeric GitHub owner, repository, and workflow IDs.

The current public fixture keeps the exact signed release bytes from
ADR-155. Only its independent trust root changes. This proves that trust
admission can be hardened without re-signing or reconstructing a release.

Missing fields, extra fields, HSM or non-Ed25519 keys, fingerprint mismatch,
private key material, non-numeric GitHub identifiers, wrong purpose, revoked
status, builder drift, workflow drift, and signature failure SHALL fail
closed.

### Dedicated staging authority

The source contract selects these staging resources:

- WIF provider:
  `projects/186366152200/locations/global/workloadIdentityPools/github/providers/cogs-publisher-stg`;
- publisher:
  `cog-release-publisher-stg@cognitum-20260110.iam.gserviceaccount.com`;
- KMS key version:
  `projects/cognitum-20260110/locations/us-central1/keyRings/cog-release-stg/cryptoKeys/release-ed25519/cryptoKeyVersions/1`;
- evidence bucket: `gs://cognitum-20260110-cog-release-stg`; and
- GitHub owner/repository/workflow IDs:
  `256911919` / `1211713542` / `322710413`.

The workflow requires the public repository, `workflow_dispatch`,
`refs/heads/main`, the exact main workflow ref, and equal release/workflow
SHAs before cloud authentication. Those source checks are defense in depth;
the WIF provider condition SHALL enforce the same claims before issuing a
token and SHALL also require the `cogs-staging` environment.

The publisher receives only public-key/version inspection,
`cloudkms.cryptoKeyVersions.useToSign`, and `storage.objects.create` on the
dedicated bucket. It receives no service-account key, object read/list,
overwrite/delete, Firestore, Secret Manager, Artifact Registry, deployment,
IAM, or key-administration authority.

The required `cogs-staging` environment variables are:

| Variable | Exact or bounded value |
|---|---|
| `GCP_COGS_STAGING_WIF_PROVIDER` | exact provider above |
| `GCP_COGS_STAGING_PUBLISH_SA` | exact publisher above |
| `GCP_COGS_STAGING_SIGNING_KEY_RESOURCE` | exact KMS version above |
| `GCP_COGS_STAGING_SIGNING_KEY_ID` | reviewed, never-reused logical key ID |
| `GCP_COGS_STAGING_EVIDENCE_BUCKET` | `cognitum-20260110-cog-release-stg` |
| `GCP_COGS_STAGING_EVIDENCE_RETENTION_SECONDS` | positive integer, at most 31,536,000 |
| `GCP_COGS_STAGING_EVIDENCE_RETENTION_LOCKED` | `false` |

Retention locking is irreversible until expiry. This Proposed ADR does not
authorize it. A future lock requires an explicit operator decision naming the
duration, cost, and recovery consequences.

### Generation-bound evidence admission

Every upload remains create-only. The workflow additionally captures the
generation returned by the object-creation response. It emits
`cognitum.cog.release-evidence-locations.v1` for the exact
`release-evidence.json` URI, positive generation, SHA-256 content digest, and
`ifGenerationMatch: 0`.

The admission document declares public-access prevention, uniform
bucket-level access, versioning, retention duration, and lock state. It is a
handoff, not self-attestation: the website ingestion identity independently
reads the live bucket controls, exact generation, and exact bytes. The legacy
bucket, mutable aliases, generation zero, digest/path mismatch, missing
protections, excess retention, or ambiguous objects fail closed.

The workflow preserves the admission document as a short-lived GitHub
artifact and also writes it to a create-only content-addressed object. Neither
copy automatically enters the website allowlist.

### Per-digest signed withdrawal

`scripts/cog_release_provenance.py` supplies source-only commands to:

1. verify an original signed release under trust v2;
2. prepare a canonical `cognitum.cog.release-withdrawal.v1` statement;
3. sign its raw bytes externally with KMS Ed25519;
4. finalize a separately signed withdrawal evidence wrapper; and
5. verify its release digest, original release payload digest, Cog ID, action,
   reason, issuance time, key purpose, issuer, numeric GitHub identity, and
   signature.

The original release is never edited. A malformed or unverifiable withdrawal
must be treated as a deployment refusal, not as absence.

This change does not create a live withdrawal workflow. Such a workflow needs
an independently reviewed way to read one exact retained release generation
without giving the create-only publisher broad object-read authority.

## Required validation

- Python and the website Node verifier accept the same trust v2 fixture.
- The signed release fixture remains byte-for-byte unchanged.
- KMS resource, algorithm, protection, fingerprint, purpose, numeric ID,
  builder, workflow, and signature mutations fail.
- Withdrawal round-trip verification passes; record, binding, purpose, issuer,
  and signature mutations fail.
- Dedicated-bucket admission passes; legacy bucket, generation zero,
  digest/path mismatch, missing protection, and mutable upload mutations fail.
- All six release schemas and four integration schemas resolve locally.
- Workflow syntax and static mutation tests pass.
- No production workflow gains signing, bucket, withdrawal, or deployment
  authority.
- A real protected staging run, website ingestion, create-only Firestore seed,
  runtime release/withdrawal refusal, and redacted receipts remain mandatory.

## Rollout

1. Merge and independently review this source-only contract.
2. Protect `main` with independent approval and no routine role bypass.
3. Create `cogs-staging` with required reviewers and no self-approval.
4. Enable STS and create the dedicated keyless publisher and claim-restricted
   provider.
5. Create the SOFTWARE Ed25519 key and dedicated protected bucket; do not lock
   retention without a separate approval.
6. Run negative WIF/IAM/KMS/Storage canaries before setting environment
   variables.
7. Publish one zero-production-authority staging release.
8. Independently read back bucket controls, generation, and content digest;
   merge the reviewed trust entry into website staging.
9. Seed and exercise one staging release, then a separately signed withdrawal.
10. Keep production frozen until both repositories' production ADR gates are
    separately accepted.

Rollback before the first signature disables federation and removes the
publisher bindings. After signing, evidence remains immutable; containment
disables new signing and publishes a separately signed withdrawal. It never
edits or deletes the original release to simulate rollback.

## Consequences

- The existing signed release format remains stable while its authority
  metadata becomes explicit and verifiable.
- Publisher compromise is bounded to new create-only staging objects and use
  of one signing version; it cannot mutate retained evidence or deploy.
- The website can ingest exact producer evidence without trusting producer
  claims about live storage state.
- Cloud provisioning, GitHub protection, a withdrawal execution identity, and
  staging proof remain external prerequisites. Source tests alone do not make
  this deployable.

## Alternatives considered

- **Keep trust v1** — rejected because it cannot prove the KMS resource,
  protection level, key fingerprint, purpose, or numeric repository identity.
- **Continue using `cognitum-apps`** — rejected because a public, object-admin
  bucket is not release-authority custody.
- **Mark the release itself withdrawn** — rejected because it mutates signed
  history and creates ambiguous restore authority.
- **Use key revocation for one digest** — rejected because it globally
  invalidates every release signed by the key.
- **Grant the publisher object viewer/admin** — rejected because generation is
  returned by create and independent readback belongs to the consumer/auditor.
