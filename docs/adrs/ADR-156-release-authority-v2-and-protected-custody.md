# ADR-156: Quorum-rooted release authority and separated staging custody

**Status:** Superseded for routine releases
**Date:** 2026-07-29
**Updated:** 2026-08-31 (routine-release topology retired)
**Founder-stage repository release override:** Recorded; enforcement is owned by each release lane
**Production deployment:** NOT approved
**Staging authority:** NO-GO until every required evidence item is complete
**Irreversible retention lock:** NOT approved by this ADR
**Website authority:** ADR-128 in `cognitum-one/website`

## Supersession notice (2026-08-31)

Website ADR-140 now governs routine Cog releases and supersedes this proposed
topology for that purpose. The quorum-rooted trust registry, evidence seeders,
reconciliation auditor, receipt chain, deployment admission, and artifact
custody described below were never activated and grant no release authority.

The routine artifact-integrity boundary is deliberately lean:

1. an authorized team member deliberately invokes publication after the
   applicable exact-head automated checks pass;
2. signing is isolated from pull-request code and uses a non-exportable,
   purpose-bound key;
3. publication produces immutable, digest-addressed artifacts and evidence;
   and
4. a digest-specific withdrawal or quarantine action remains available for
   containment, followed by monitoring, forward-fix publication, and consumer
   pin rollback.

No mandatory second human, independent review, branch promotion, or technical
quorum applies to the routine lane. The remainder of this ADR is retained as
historical design rationale for a deliberately selected high-assurance lane.

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

## Founder-stage human approval override (2026-08-15)

The team is currently too small to assign the human custodian, reviewer,
publisher, merger, and deployment-operator roles to different people. Dragan
explicitly overrides that natural-person separation during the founder stage.
This does not collapse the authority topology above: cryptographic keys,
service accounts, workload-identity providers, protected environments,
databases, buckets, purpose restrictions, immutable artifacts, receipts,
audit records, and rollback boundaries remain separate technical principals
and controls.

The authorized founder-stage approvers are identified by GitHub login and
immutable numeric user ID:

- Dragan Spiridonov — `proffesor-for-testing` / `214853444`;
- rUv — `ruvnet` / `2934394`;
- Nick Ruest — `nicholas-ruest` / `127058086`;
- Ofer Shaal — `shaal` / `22901`;
- Rob Ranson — `rcraw` / `61807077`; and
- Martin Vladimirov — `martinvlad` / `36089262`.

Any one listed person MAY author, review, approve, merge, stage, and promote the
same pull request. Self-approval is explicit and must not be represented as an
independent review or multi-person quorum. Release approval is valid only when
automation binds the allowlisted login and numeric ID to the exact PR head and
resulting merge SHA, all required CI is green, at least one exact-head
adversarial review records a no-block verdict, the full QE PR flow is complete,
the relevant staging evidence is successful, the immutable artifact and
rollback target are identified, and the approval is recorded before deployment
credentials are acquired.

This section supersedes every natural-person-separation, two-human-reviewer,
no-self-approval, independent-human-review, and human-quorum statement in this
ADR while the founder-stage override is active. Distinct cryptographic keys and
signatures may still be required as technical integrity and purpose-separation
controls. If one person operates more than one such role, those signatures MUST
be described as distinct technical signatures, not independent human approvals.

This override prevents the absent multi-person team from blocking ordinary
repository PRs and otherwise-authorized staging or production releases. It does
not synthesize the missing Cog authority evidence, approve retention locking,
or lift the ADR-155 production publishing freeze. Those controls remain
fail-closed until their own stated evidence and approval conditions are met.
When staffing permits independent custody, replace this temporary override
through the same full QE PR flow.

## Root bootstrap and trust registry

The immutable `cognitum.cog.trust-bootstrap.v1` production document SHALL
contain exactly these three IDs, their exact versioned non-exportable signing
resources, SPKI SHA-256 fingerprints, and `threshold: 2`:

- `security-custodian/cog-trust-root-a`;
- `platform-custodian/cog-trust-root-b`; and
- `independent-auditor/cog-trust-root-c`.

During the founder-stage override one allowlisted person may operate more than
one role, but the named roles, keys, signatures, service accounts, and audit
records remain distinct. GitHub Actions, publisher/seeder service accounts,
runtime identities, and the evidence project cannot access root private
material or mutate the bootstrap.

This repository contains the proposed public bootstrap only at the canonical
path `config/cog-trust-bootstrap.v1.json`. It records the three exact
versioned resources, public keys, and independently read-back SPKI
fingerprints, but it is not live authority: non-overlapping custodian IAM,
source-digest approval, the runtime pin, and receipt evidence are incomplete.
The former `config/cog-trust-bootstrap.json` path is forbidden. The fixture at
`tests/fixtures/cog-release/trust-bootstrap.json` is test-only and is not a
production trust anchor. Until three real custodians and a reviewed source
digest exist, there is no admitted genesis registry.

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
Accepted successor ADR, staging rollback proof, and founder-gate approval.

## Purpose isolation and authority topology

Registry entries have exactly one purpose and reside in the matching array.
Release verification searches only `releases`; withdrawal verification
searches only `withdrawals`. A release-purpose key on a withdrawal path and a
withdrawal-purpose key on a release path are rejected even if the key,
signature, workflow, and KMS resource would otherwise be valid. The two
publishers must also use different logical key IDs and KMS versions.

The historical proposed staging target was:

| Act | Principal / provider | Key / workflow / environment | Protected destination |
|---|---|---|---|
| Trust append | `cog-trust-append-stg@cognitum-20260110.iam.gserviceaccount.com`; `github-cog-authority-stg/providers/cogs-trust-registry-appender-stg` | retired without activation | create-only `gs://cognitum-20260110-cog-trust-stg` |
| Release publish | `cog-release-publisher-stg@cognitum-20260110.iam.gserviceaccount.com`; `github-cog-authority-stg/providers/cogs-release-publisher-stg` | `cog-release-stg/cryptoKeys/release-ed25519/cryptoKeyVersions/1`; `.github/workflows/publish-cog-staging.yml`; `cogs-staging` | `gs://cognitum-20260110-cog-release-stg` |
| Withdrawal publish | `cog-withdrawal-publisher-stg@cognitum-20260110.iam.gserviceaccount.com`; `github-cog-authority-stg/providers/cogs-withdrawal-publisher-stg` | `cog-withdrawal-stg/cryptoKeys/withdrawal-ed25519/cryptoKeyVersions/1`; `.github/workflows/withdraw-cog-staging.yml`; `cogs-withdrawal-staging` | `gs://cognitum-20260110-cog-withdrawal-stg` |
| Release seed | requester `cog-rel-seed-req-stg@cognitum-20260110.iam.gserviceaccount.com`; runtime `cog-rel-seed-run-stg@cognitum-20260110.iam.gserviceaccount.com`; `website-release-seeder-stg` | website `seed-cog-release-staging.yml`; `cogs-release-seed-staging` | create-only named Firestore database `cog-release-staging` |
| Withdrawal seed | requester `cog-wd-seed-req-stg@cognitum-20260110.iam.gserviceaccount.com`; runtime `cog-wd-seed-run-stg@cognitum-20260110.iam.gserviceaccount.com`; `website-withdrawal-seeder-stg` | website `seed-cog-withdrawal-staging.yml`; `cogs-withdrawal-seed-staging` | create-only named Firestore database `cog-withdrawal-staging` |
| Reconcile | requester `cog-rel-audit-req-stg@cognitum-20260110.iam.gserviceaccount.com`; runtime `cog-rel-audit-run-stg@cognitum-20260110.iam.gserviceaccount.com`; `website-release-auditor-stg` | website `reconcile-cog-seed-staging.yml`; `cogs-reconciliation-staging`; read-only | exact `cog-release-staging` / `cog-withdrawal-staging` projection, evidence, and receipt reads |
| Receipt attest | `cog-receipt-attestor-stg@cognitum-20260110.iam.gserviceaccount.com` | `cog-receipt-stg/cryptoKeys/receipt-ed25519/cryptoKeyVersions/1` | `gs://cognitum-20260110-cog-receipts-stg` |

All KMS versions use `EC_SIGN_ED25519` and `SOFTWARE`. The trust appender and
two publisher workflows in this repository are source candidates for their
three named acts only. The website seeders, auditor, receipt attestor, and
runtime changes are external prerequisites and remain NO-GO blockers.

The `cogs-withdrawal-staging` GitHub environment SHALL require the founder-stage
release gate above. Until the override is retired, its two required technical
approval signatures may be produced by one allowlisted person; self-approval
is permitted but administrator bypass of the evidence gate is not. Its
immutable approval receipt binds the approver identities,
change ID, workflow SHA, run attempt, release digest, and withdrawal digest.
Source comments cannot prove that environment configuration.

The source-only trust appender consumes exactly
`trust-admissions/<change-id>/registry.json` and, after genesis, the exact
`previous-registry.json` in the same reviewed directory. No other file is
allowed in that package. The registry must already contain two or three valid
root signatures and use the deterministic reviewed JSON serialization. The
appender verifies the canonical bootstrap digest, the protected current head
sequence and digest, one appended purpose, the purpose-specific KMS resource,
SPKI fingerprint, numeric owner/repository and workflow IDs, and the exact
publisher workflow SHA. Its fixed
predecessor/sequence destination plus `if-generation-match=0` prevents two
successors from occupying the same admitted sequence.

The appender's own workflow SHA, numeric workflow ID, approved registry
digest, bootstrap digest, and current registry head are protected environment
values. They are deliberately unset until external review completes. The
workflow does not create root signatures, read or list trust storage, update
those values, rotate a WIF condition, or seed runtime state. Any provider SHA
change still requires the out-of-band federation-admin process and a digest
of that approval is bound into the append plan.

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
verified, founder-gate-approved withdrawal seeder may create a projection. Its
two technical approval signatures do not imply two human reviewers during the
founder stage.

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
workflow SHA, through an out-of-band audited GCP request with two technical
approval signatures and the dedicated `cog-federation-admin` service account.
During the founder stage one allowlisted person may create both signatures;
the request references
the quorum-signed registry update, proves old/new deny canaries and rollback,
and never temporarily broadens the condition. Publishers, seeders, runtime
identities, GitHub workflows, and repository administrators have no
provider-update permission.

The admission workflow cannot approve its own SHA. It requires the exact
`github.workflow_sha` to equal both `github.sha` on `refs/heads/main` and a
separately set protected-environment SHA. It records a numeric workflow ID and
workflow-SHA approval-receipt digest, while the WIF provider independently
pins the exact workflow reference and SHA. A source commit alone cannot
populate or rotate those external pins.

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
authoritative `NOT_FOUND` permits a new founder-gate-approved attempt.
`MISMATCH` quarantines the digest. The current release, withdrawal, and
trust-append workflows stop as `UNKNOWN`, but their local artifacts are not
yet receipt-attestor signed or stored in the protected receipt chain; the
external auditor does not yet reconcile GCS object creates. Those are explicit
blockers, not successful idempotency.

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

## Historical deterministic validation evidence

The removed source candidate included an EV-128-01 through EV-128-14 owner and
artifact matrix. It assigned each item an owner, reviewer role, command,
expected result, source implementation status, and deterministic artifact
path:

`evidence/adr-156/<run-id>/<evidence-id>.json`

Each envelope contains start/finish UTC timestamps, source SHA, subject
resource/version/digest, exact command and tool versions, expected/actual
result, exit code, input/stdout/stderr SHA-256 digests, workflow identity and
attempt, redaction declaration, reviewer role, and previous receipt digest.
One allowlisted person may fill both owner and reviewer roles during the
founder stage, but the evidence MUST record that overlap. The receipt attestor
signs the canonical envelope and appends it to the
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

Rollout order is: source contract; real bootstrap/custodian roles; branch and
founder-gated environments; key-prohibition policies; separate keyless
identities/providers; three KMS keys; three protected buckets; quorum-signed
registry; negative federation/IAM canaries; website parser/cache migration;
verified seeders and read-only auditor; signed receipt chain; authenticated
staging E2E; founder-stage QE approval; then a separate production decision.

Before the first admitted release, rollback disables the provider and service
account, removes workload bindings and bucket-create roles, and disables the
KMS version. After signing, original evidence is not edited or deleted.
Containment removes future authority and publishes a separately signed
per-digest withdrawal.

Implemented in this source candidate:

- canonical proposed public bootstrap path
  `config/cog-trust-bootstrap.v1.json`, with the legacy path rejected;
- trust bootstrap v1 and quorum-signed, chained registry v3 validation;
- a protected, source-only trust-admission workflow candidate that accepts
  only an already 2-of-3-signed single append, binds the exact numeric and
  workflow-SHA topology, uses one predecessor/sequence create-only path, and
  treats every ambiguous result as terminal `UNKNOWN`;
- dedicated release/withdrawal purpose arrays and high-level cross-purpose
  rejection;
- bounded signed release-validity and withdrawal statements;
- separate release/withdrawal publisher workflow candidates;
- create-only GCS publication with terminal `UNKNOWN` handling;
- transparency-bearing evidence admission;
- exact runtime cache policy; and
- baseline plus adversarial source tests and a deterministic evidence matrix.

Still missing and therefore NO-GO:

- verified, technically non-overlapping root-custodian IAM and founder-gate
  approval of the proposed source bootstrap digest;
- quorum-signed live registry and exact workflow-SHA rotation governance;
- all target GCP identities, providers, keys, buckets, IAM, organization
  policies, audit logs, and GitHub environment protections;
- verified release/withdrawal seeders, read-only auditor, receipt attestor, and
  transparency chain;
- website registry-v3/validity/cache parser migration;
- retained reconciliation of ambiguous object and Firestore creates;
- authenticated staging and GCP E2E evidence EV-128-01 through EV-128-14; and
- an Accepted production ADR.

ADR-156 is **Superseded for routine releases**. Its proposed staging authority
was never activated, production was never approved by this ADR, and retention
locking remains **NOT approved**.
