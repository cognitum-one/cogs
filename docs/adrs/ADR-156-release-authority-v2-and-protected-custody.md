# ADR-156: Quorum-rooted release authority and separated staging custody

**Status:** Proposed
**Date:** 2026-07-29
**Production deployment:** NOT approved
**Staging authority:** NO-GO until every required evidence item is complete
**Irreversible retention lock:** NOT approved by this ADR
**Website authority:** ADR-128 in `cognitum-one/website`

This ADR supersedes the staging trust-admission and evidence-custody portions
of ADR-154. ADR-154's canonical Ed25519 release record and ADR-155's signed
runtime-integration binding remain in force.

## Context

A built Cog, catalogue entry, OAuth grant, API key, reachable demo, or model
response is not deployment authority. Deployment authority exists only after a
release is signed by an admitted release key, its bounded validity statement is
current, its evidence is immutable, and no effective or malformed withdrawal
projection applies.

The earlier staging candidate used a self-describing trust registry. That made
the registry capable of admitting its own keys and did not provide an
independent withdrawal authority. It also left ambiguous object creates,
runtime cache expiry, receipt integrity, service-account key policy, and
workflow-SHA rotation underspecified.

This decision aligns the Cogs repository with website ADR-128. It is a source
contract only. No GCP resource, GitHub provider/environment protection,
production bootstrap, signed registry, Firestore projection, or production
deployment is created or approved by this ADR.

## Decision

Release authority SHALL remain unavailable until the following independent
acts are implemented and evidenced:

1. three offline custodians establish a source-pinned 2-of-3 trust bootstrap;
2. two custodians admit purpose-specific publisher keys in an append-only
   trust registry;
3. a release publisher signs and creates immutable release evidence;
4. a different withdrawal publisher signs and creates immutable withdrawal
   evidence;
5. separate verified release and withdrawal seeders create projections;
6. a read-only auditor reconciles ambiguous creates;
7. a dedicated receipt attestor signs an append-only receipt chain; and
8. the runtime admits deployments only through the pinned bootstrap, current
   registry, current release validity, verified projections, and bounded cache.

The release publisher, withdrawal publisher, release seeder, withdrawal
seeder, read-only auditor, receipt attestor, trust custodians, federation
administrator, and runtime SHALL be separate principals. A publisher cannot
admit its own key, seed a projection, update a provider, or sign for the other
purpose. A seeder cannot sign. The auditor cannot write or retry. The runtime
has no signing credential.

## Root bootstrap and trust registry

The immutable `cognitum.cog.trust-bootstrap.v1` production document SHALL
contain exactly these three IDs, their exact versioned non-exportable signing
resources, SPKI SHA-256 fingerprints, and `threshold: 2`:

- `security-custodian/cog-trust-root-a`;
- `platform-custodian/cog-trust-root-b`; and
- `independent-auditor/cog-trust-root-c`.

One person cannot occupy more than one role. GitHub Actions, publisher/seeder
service accounts, runtime identities, and the evidence project cannot access
root private material or mutate the bootstrap.

This repository deliberately does not contain
`config/cog-trust-bootstrap.json`. The fixture at
`tests/fixtures/cog-release/trust-bootstrap.json` is test-only and is not a
production trust anchor. Until three real custodians, resources, fingerprints,
and a reviewed source digest exist, there is no production genesis registry.

Every `cognitum.cog.trust-registry.v3` statement contains:

- a strictly increasing `sequence`;
- `GENESIS` for sequence 1, otherwise the previous registry payload digest;
- whole-second UTC `issuedAt`, `notBefore`, and `expiresAt`;
- separate, exact `releases` and `withdrawals` arrays;
- each key's exact versioned KMS resource, SPKI fingerprint, validity, numeric
  GitHub identity, immutable workflow reference, and reviewed workflow SHA;
- explicit revocation effective time, reason, and scope; and
- two or three detached Ed25519 root signatures over the canonical registry.

The registry is configuration, not the root of trust. A GCS object, Firestore
document, authenticated response, environment variable, or registry signature
by an admitted publisher cannot admit a key. Runtime verification requires the
compiled bootstrap digest, at least two distinct bootstrap roles, the compiled
minimum sequence, the expected registry digest, continuity with the prior
registry, and current time bounds.

One signature, duplicate roles, an unknown root, a substituted bootstrap
fingerprint, a broken signature, a sequence gap, a fork, a rollback, an expired
registry, or a broken predecessor digest fails closed. Root rotation requires
an old-root quorum transition, a new runtime image and bootstrap digest, an
Accepted successor ADR, staging rollback proof, and independent approval.

## Purpose isolation and authority topology

Registry entries have exactly one purpose and reside in the matching array.
Release verification searches only `releases`; withdrawal verification
searches only `withdrawals`. A release-purpose key on a withdrawal path and a
withdrawal-purpose key on a release path are rejected even if the key,
signature, workflow, and KMS resource would otherwise be valid. The two
publishers must also use different logical key IDs and KMS versions.

The approved staging target is:

| Act | Principal / provider | Key / workflow / environment | Protected destination |
|---|---|---|---|
| Release publish | `cog-release-publisher-stg@cognitum-20260110.iam.gserviceaccount.com`; `github-cog-authority-stg/providers/cogs-release-publisher-stg` | `cog-release-stg/cryptoKeys/release-ed25519/cryptoKeyVersions/1`; `.github/workflows/publish-cog-staging.yml`; `cogs-staging` | `gs://cognitum-20260110-cog-release-stg` |
| Withdrawal publish | `cog-withdrawal-publisher-stg@cognitum-20260110.iam.gserviceaccount.com`; `github-cog-authority-stg/providers/cogs-withdrawal-publisher-stg` | `cog-withdrawal-stg/cryptoKeys/withdrawal-ed25519/cryptoKeyVersions/1`; `.github/workflows/withdraw-cog-staging.yml`; `cogs-withdrawal-staging` | `gs://cognitum-20260110-cog-withdrawal-stg` |
| Release seed | `cog-release-seeder-stg@cognitum-20260110.iam.gserviceaccount.com`; `website-release-seeder-stg` | website `seed-cog-release-staging.yml`; protected release-seeder environment | named Firestore database `staging` |
| Withdrawal seed | `cog-withdrawal-seeder-stg@cognitum-20260110.iam.gserviceaccount.com`; `website-withdrawal-seeder-stg` | website `seed-cog-withdrawal-staging.yml`; two-person withdrawal environment | named Firestore database `staging` |
| Reconcile | `cog-release-auditor-stg@cognitum-20260110.iam.gserviceaccount.com`; `website-release-auditor-stg` | website `reconcile-cog-seed-staging.yml`; read-only | exact projection/evidence/receipt reads |
| Receipt attest | `cog-receipt-attestor-stg@cognitum-20260110.iam.gserviceaccount.com` | `cog-receipt-stg/cryptoKeys/receipt-ed25519/cryptoKeyVersions/1` | `gs://cognitum-20260110-cog-receipts-stg` |

All KMS versions use `EC_SIGN_ED25519` and `SOFTWARE`. The publisher workflows
in this repository are candidates for the first two acts only. The website
seeders, auditor, receipt attestor, and runtime changes are external
prerequisites and remain NO-GO blockers.

The `cogs-withdrawal-staging` GitHub environment SHALL require two distinct
Cognitum Security/Platform reviewers, prevent self-approval, and prevent
administrator bypass. Its immutable approval receipt binds the reviewers,
change ID, workflow SHA, run attempt, release digest, and withdrawal digest.
Source comments cannot prove that environment configuration.

## Time and signature contract

The existing signed release v1 bytes remain stable. A signed
`cognitum.cog.release-validity.v1` authority sidecar binds the exact release
digest, signed release payload digest, Cog ID, and:

- `issuedAt`;
- `signedAt`;
- `notBefore`; and
- `expiresAt`.

All timestamps are whole-second RFC 3339 UTC with `Z`. Clock skew is exactly
300 seconds. `notBefore` must be within 300 seconds of `issuedAt`;
`issuedAt <= signedAt <= notBefore + 300s`; `expiresAt` is later than
`notBefore`; and `expiresAt - issuedAt` is at most 30 days. A release is not
current before its skew-bounded start or when
`checkedAt - 300s >= expiresAt`. `signedAt` must also fall inside the admitted
release key interval.

The sidecar preserves already-published release bytes while adding the temporal
authority ADR-128 requires. It is mandatory, signed by the exact release key,
and cannot be synthesized from catalogue metadata.

A `cognitum.cog.release-withdrawal.v1` statement binds the original release
digest and signed payload digest, Cog ID, action, reason, issuer identity,
reviewed workflow SHA, `issuedAt`, and `effectiveAt`. Its key must be admitted
only for `withdrawal`. `effectiveAt` cannot predate the bound release
`issuedAt`, cannot be later than withdrawal `issuedAt`, and never expires.
Once effective, the exact digest is permanently denied.

A malformed, ambiguous, unknown-key, wrong-purpose, invalid-signature, or
wrong-binding immutable withdrawal is never interpreted as absence. It
quarantines its exact bound release digest pending read-only audit. It does not
re-enable that release and cannot globally deny unrelated digests. Only the
verified, two-person-approved withdrawal seeder may create a projection.

## Website parser and runtime migration

The current website trust-v2 parser is incompatible with this contract. Before
any deployment authority is enabled, `cognitum-one/website` SHALL:

1. embed and source-pin the production bootstrap digest;
2. parse registry v3, verify 2-of-3 roots, sequence, predecessor digest, time
   bounds, key-purpose arrays, workflow SHA, and revocation scope;
3. ingest and require the signed release-validity sidecar;
4. enforce withdrawal `effectiveAt` and exact-digest quarantine semantics;
5. keep catalogue metadata separate from deployability;
6. use the cache policy in
   `config/cog-release-runtime-cache-policy.json`; and
7. emit signed receipts for all admission and reconciliation decisions.

Until those parser and ingestion changes are merged, deployed, and proven,
the website must expose zero newly deployable Cogs. Cogs source tests cannot
stand in for website parser or authenticated runtime evidence.

## Federation and workflow-SHA authority

The release and withdrawal providers bind numeric owner ID `256911919`,
numeric Cogs repository ID `1211713542`, one exact workflow reference and SHA,
`refs/heads/main`, `workflow_dispatch`, their distinct protected environment,
and their distinct service account. Cross-provider token exchange, pull
requests, forks, alternate claims, mutable repository-name-only conditions,
and publisher provider updates fail.

Only `cog-trust-federation-admins@cognitum.one` may change a provider's exact
workflow SHA, through an out-of-band audited GCP request with two approvers and
the dedicated `cog-federation-admin` service account. The request references
the quorum-signed registry update, proves old/new deny canaries and rollback,
and never temporarily broadens the condition. Publishers, seeders, runtime
identities, GitHub workflows, and repository administrators have no
provider-update permission.

Every authority service account is keyless and has zero user-managed keys.
Organization/folder policy SHALL enforce both, with no project exception:

- `constraints/iam.disableServiceAccountKeyCreation`; and
- `constraints/iam.disableServiceAccountKeyUpload`.

EV-128-11 enumerates every authority service account and proves both policies,
zero user-managed keys, cross-act IAM denial, and required Data Access audit
logs.

## Immutable create and ambiguous outcomes

Release and withdrawal evidence uses a content-addressed object name and
`if-generation-match=0`. Firestore projections use `createDocument` in the
explicit `staging` database; `PATCH`, update, merge, upsert, overwrite, delete,
or method-changing retry is forbidden.

A timeout, reset, 408, 429, 5xx, lost response, client termination, or missing
positive GCS generation after a create is `UNKNOWN`, because the create may
have committed. The publisher or seeder:

1. emits a retained canonical `UNKNOWN` attempt receipt binding request bytes,
   digest, destination, time, workflow attempt, and transport result;
2. marks `retryAllowed: false`;
3. stops without retry, upsert, alternate ID, read, or success claim; and
4. opens a reconciliation item for the separate read-only auditor.

The auditor reads only the exact destination and returns a signed,
append-only `COMMITTED`, `NOT_FOUND`, or `MISMATCH` receipt. Only an
authoritative `NOT_FOUND` permits a new two-person-approved attempt.
`MISMATCH` quarantines the digest. The current Cogs workflows stop as
`UNKNOWN`, but their local artifact is not yet receipt-attestor signed or
stored in the protected receipt chain; the external auditor does not yet
reconcile GCS object creates. Those are explicit blockers, not successful
idempotency.

## Evidence custody and integrity

The three buckets use public-access prevention, uniform bucket-level access,
versioning, object-create-only publisher roles, separate read-only auditors,
and positive retention. This Proposed ADR sets a 30-day positive, unlocked
staging retention candidate. A retention lock is irreversible and requires a
separate explicit approval naming duration, cost, and recovery effects.

Sigstore evidence is acceptable only when the bundle contains transparency-log
material with an inclusion promise or proof. Evidence-location admission binds
the exact bucket, object, positive generation, content digest, KMS signature
verification, registry-quorum requirement, Sigstore bundle digest, and
transparency verification.

Every attempt and reconciliation receipt SHALL be detached-signed by the
receipt attestor and appended to a sequence/digest-linked transparency chain
with signed checkpoints. Missing signatures, broken links, rollback, duplicate
sequence, unsigned `UNKNOWN`, or retention failure blocks release. This
receipt-attestor and chain are not implemented in this repository and remain a
NO-GO prerequisite.

## Runtime cache and outages

The runtime does not perform live KMS metadata lookups. KMS metadata is bound
during trust admission.

The exact cache contract is:

- source-pinned bootstrap: no network TTL;
- validated registry: 300 seconds;
- verified release projection: 30 seconds;
- verified withdrawal projection and negative lookup: 30 seconds.

A cold start without all initial dependencies exposes zero deployable
releases. During an outage, a new deployment may use only a still-fresh,
fully-verified cache entry. After either TTL, new deploy, scale-up, clone,
upgrade, and placement deny as dependency unavailable and require
`Retry-After`. Existing running workloads continue solely because of metadata
dependency failure. A valid or malformed withdrawal immediately denies or
quarantines the exact digest and cannot be overridden by an older cache.

## Deterministic validation evidence

The machine-readable owner and artifact matrix is
`config/cog-release-evidence-matrix.v1.json`. It mirrors website EV-128-01
through EV-128-14 and assigns each item an owner, independent reviewer, exact
command, expected result, source implementation status, and deterministic
artifact path:

`evidence/adr-156/<run-id>/<evidence-id>.json`

Each envelope contains start/finish UTC timestamps, source SHA, subject
resource/version/digest, exact command and tool versions, expected/actual
result, exit code, input/stdout/stderr SHA-256 digests, workflow identity and
attempt, redaction declaration, independent reviewer, and previous receipt
digest. The receipt attestor signs the canonical envelope and appends it to the
receipt chain. Console prose and screenshots are supporting material, not gate
evidence.

The matrix commands that do not exist or cannot yet produce signed evidence
are explicit implementation blockers. They cannot be waived by marking this
ADR Accepted.

## Required adversarial proof

The source and live evidence together SHALL prove:

- one, duplicate, unknown, or invalid root signatures fail;
- source-bootstrap substitution, fingerprint mismatch, registry rollback,
  expiry, fork, predecessor mismatch, and sequence gap fail;
- release and withdrawal purpose, identity, workflow, provider, environment,
  service account, KMS key, and bucket cannot be exchanged;
- missing transparency-log material, tampered validity, overlong lifetime,
  expired validity, and invalid signing times fail;
- `effectiveAt` before release issuance or after withdrawal issuance fails;
- malformed immutable withdrawals quarantine one exact digest and are never
  absence;
- 300/30-second TTL widening and fail-open refresh behavior fail;
- ambiguous create is `UNKNOWN`, never retryable, and never upserted;
- SA user-managed key creation/upload and cross-act IAM fail; and
- a catalogue record without the complete authority chain yields no deploy.

## Rollout, rollback, and current status

Rollout order is: source contract; real bootstrap/custodians; branch and
two-person environments; key-prohibition policies; separate keyless
identities/providers; three KMS keys; three protected buckets; quorum-signed
registry; negative federation/IAM canaries; website parser/cache migration;
verified seeders and read-only auditor; signed receipt chain; authenticated
staging E2E; independent review; then a separate production decision.

Before the first admitted release, rollback disables the provider and service
account, removes workload bindings and bucket-create roles, and disables the
KMS version. After signing, original evidence is not edited or deleted.
Containment removes future authority and publishes a separately signed
per-digest withdrawal.

Implemented in this source candidate:

- trust bootstrap v1 and quorum-signed, chained registry v3 validation;
- dedicated release/withdrawal purpose arrays and high-level cross-purpose
  rejection;
- bounded signed release-validity and withdrawal statements;
- separate release/withdrawal publisher workflow candidates;
- create-only GCS publication with terminal `UNKNOWN` handling;
- transparency-bearing evidence admission;
- exact runtime cache policy; and
- baseline plus adversarial source tests and a deterministic evidence matrix.

Still missing and therefore NO-GO:

- real root custodians and source-pinned production bootstrap;
- quorum-signed live registry and exact workflow-SHA rotation governance;
- all target GCP identities, providers, keys, buckets, IAM, organization
  policies, audit logs, and GitHub environment protections;
- verified release/withdrawal seeders, read-only auditor, receipt attestor, and
  transparency chain;
- website registry-v3/validity/cache parser migration;
- retained reconciliation of ambiguous object and Firestore creates;
- authenticated staging and GCP E2E evidence EV-128-01 through EV-128-14; and
- an Accepted production ADR.

ADR-156 remains **Proposed**, staging authority remains **NO-GO**, production
remains **NOT approved**, and retention locking remains **NOT approved**.
