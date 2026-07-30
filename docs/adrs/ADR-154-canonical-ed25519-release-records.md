# ADR-154: Canonical Ed25519 Cog release records

**Status**: Accepted
**Date**: 2026-07-29
**Superseded in part**: ADR-156 replaces the staging trust-admission and
evidence-custody handoff below; the canonical release-signature decision
remains Accepted.
**Related**: ADR-020 (build/publish), ADR-153 (optional integrations and WIF),
website ADR-113 (release evidence), website ADR-123 (runtime reconciliation)

## Context

The website release gate now verifies a detached Ed25519 signature over the
complete deployable release record. The cogs publisher already produces
valuable Sigstore keyless evidence for the binary and GitHub SLSA provenance,
but that is a different statement and signature system:

- Cosign's keyless certificate binds an ephemeral ECDSA P-256 key to the
  GitHub OIDC workflow and records it in Sigstore transparency services.
- The website contract accepts Ed25519 and signs the complete release identity,
  including operator deployment, tenancy, rollback, network, residency, and
  lifecycle declarations.

A boolean such as `signatureVerified: true`, an artifact signature, or a
signature over a partial evidence document cannot substitute for a signature
over the exact record the website will deploy.

Google Cloud KMS supports `EC_SIGN_ED25519` as PureEdDSA over raw input. It can
therefore provide the required algorithm without exporting a private key. The
existing staging publisher already uses a claim-restricted GitHub-to-Google
Workload Identity Federation identity.

The inherited feature branch also exposed two reproducibility defects: the
staging workflow silently chose `cog.toml` when it disagreed with
`Cargo.toml`, and per-Cog lockfiles were ignored while the release workflow
claimed to use `cargo build --locked`.

## Decision

### 1. Sign the exact bounded website statement

`scripts/cog_release_provenance.py` emits and verifies:

```json
{"schema":"cognitum.cog.release-provenance.v1","release":{...}}
```

The statement recursively sorts object keys, uses compact UTF-8 JSON, and has
no trailing newline. It excludes only `release.seededAt` and
`release.provenance.detachedSignature`, matching the website verifier. Signed
values are bounded, printable ASCII so Python and JavaScript key ordering and
string encoding cannot diverge. Numbers are refused because this v1 release
identity needs no numeric field and cross-runtime numeric edge cases add no
value.

The envelope is stored at `release.provenance.detachedSignature`:

```json
{
  "schema": "cognitum.cog.release-provenance.v1",
  "algorithm": "ed25519",
  "keyId": "gcp-kms:cogs-staging-release-2026-01",
  "payloadDigest": "sha256:<hex>",
  "signature": "<64-byte signature as canonical unpadded base64url>"
}
```

Schemas under `schemas/cognitum.cog.release-*.schema.json` pin the envelope,
public trust registry, and operator release policy. Unknown fields fail.

### 2. Operator declarations are source-controlled release policy

A signature cannot cover decisions that do not exist until a downstream
consumer invents them. A release-eligible Cog therefore has a ratified
`src/cogs/<id>/release-policy.json` containing its immutable blueprint binding
and deployment declarations. The publisher combines it with measured build
digests to produce the complete release record before signing.

Only `anomaly-detect` is initially ratified. Its policy exactly matches the
website declaration and immutable blueprint digest. Other Cogs remain
unreleasable until their policy, version, and lockfile are reviewed.

The stage workflow refuses every `Cargo.toml`/`cog.toml` version disagreement.
There is no dispatch override: both tracked manifests must be reconciled in
reviewed source before the signing environment can be reached.
`anomaly-detect` reconciles both manifests at the compiled `1.2.0`, opts its
generated `Cargo.lock` back into source control, and builds with `--locked`.
The remaining catalog does not receive fabricated locks or automatic version
choices.

### 3. Keep dual, non-interchangeable signature evidence

The publisher retains and verifies the pinned Cosign v3.1.2 Sigstore bundle
for the binary and the GitHub attestation bundle for build provenance. It then
asks Cloud KMS to sign the raw canonical release statement with an enabled key
version whose algorithm is exactly `EC_SIGN_ED25519`.

The KMS public key is exported, checked as Ed25519, and used locally with
OpenSSL to verify the completed record before any upload. No
`--digest-algorithm` is supplied: PureEdDSA signs the raw statement. The final
build evidence continues to describe the artifact signature as
`ecdsa-p256`/`sigstore-bundle`; `releaseSignature` separately describes the
Ed25519 release-record signature.

The vulnerability scan, pinned Syft SBOM, dependency lock, Sigstore
verification, native isolation run with a negative control, SLSA provenance,
and policy decision all complete before KMS is called. Missing or failed
evidence prevents signing.

### 4. KMS signing is stage-only until evidence is reviewed

`.github/workflows/publish-cog-staging.yml` remains manual-only and uses the
protected `cogs-staging` environment. Production workflows receive no KMS
variables or Ed25519 signing authority in this ADR.

The staging environment requires:

| Variable | Meaning |
|---|---|
| `GCP_COGS_STAGING_WIF_PROVIDER` | claim-restricted WIF provider resource |
| `GCP_COGS_STAGING_PUBLISH_SA` | dedicated staging publisher service account |
| `GCP_COGS_STAGING_SIGNING_KEY_RESOURCE` | exact KMS CryptoKeyVersion resource |
| `GCP_COGS_STAGING_SIGNING_KEY_ID` | stable logical registry key id |

Every value is syntax-validated before authentication. The workflow verifies
the KMS algorithm and state instead of trusting variable names. It uses
create-only GCS uploads under the staging prefix and cannot write a production
or legacy mutable path.

The service account receives only:

- create-only access to the staging release object prefix;
- `cloudkms.cryptoKeyVersions.useToSign` on the staging signing key; and
- `cloudkms.cryptoKeyVersions.viewPublicKey` on that key.

It receives no private key, key administration, key-version creation, secret
access, object deletion, production bucket, IAM, or deployment authority.

### 5. The handoff artifact is public-only and workflow-bound

The original v1 design emitted a self-describing public registry entry. ADR-156
supersedes that trust-admission path. The staging workflow now emits an
explicitly unsigned `candidate-trust-registry.json`; it has no authority until
two independent source-pinned roots sign an append-only
`cognitum.cog.trust-registry.v3` statement. A website operator must not merge
the candidate directly into runtime trust.

The signed `release-evidence.json`, `signed-release.json`, and mandatory signed
release-validity sidecar remain the release-record handoff. Trust admission is
independent and quorum-rooted.

### 6. External provisioning and rotation procedure

Provisioning is an operator action outside the workflow. With reviewed values
substituted for the shell variables:

```bash
gcloud kms keyrings create cogs-release-staging \
  --project "$GCP_PROJECT" --location "$KMS_LOCATION"

gcloud kms keys create cogs-release-staging \
  --project "$GCP_PROJECT" --location "$KMS_LOCATION" \
  --keyring cogs-release-staging \
  --purpose asymmetric-signing \
  --default-algorithm ec-sign-ed25519 \
  --protection-level software

gcloud kms keys add-iam-policy-binding cogs-release-staging \
  --project "$GCP_PROJECT" --location "$KMS_LOCATION" \
  --keyring cogs-release-staging \
  --member "serviceAccount:$STAGING_PUBLISH_SA" \
  --role roles/cloudkms.signer

gcloud kms keys add-iam-policy-binding cogs-release-staging \
  --project "$GCP_PROJECT" --location "$KMS_LOCATION" \
  --keyring cogs-release-staging \
  --member "serviceAccount:$STAGING_PUBLISH_SA" \
  --role roles/cloudkms.publicKeyViewer
```

The WIF provider condition must retain ADR-153's numeric repository/owner,
manual event, exact workflow, protected environment, ref, and audience
restrictions. The four environment variables are set only after those IAM
bindings and conditions are reviewed.

For routine rotation:

1. create a new version on the existing asymmetric key;
2. choose a never-reused logical key id and export its public key;
3. add the new public entry to website staging while the old entry remains
   `active`;
4. switch both staging KMS variables to the new version/id and complete a
   verified staging release;
5. stop using the old version for signing, but keep its public entry active
   while retained releases depend on it; and
6. mark an old key `revoked` only for compromise or explicit distrust.

The v1 website registry has no `verify-only` status. Marking a healthy rotated
key `revoked` would intentionally invalidate its historical releases, so
ordinary rotation keeps the old public key active for verification. KMS
disablement/destruction follows the evidence retention policy and is never
performed by the publisher.

## Required validation

- Python and Node produce byte-identical canonical statements.
- An ephemeral local Ed25519 key signs and verifies a complete release record.
- Payload, declaration, key, builder, workflow, and signature tampering fail.
- A non-Ed25519 public key and a non-64-byte signature fail.
- The final evidence retains verified Sigstore metadata independently.
- Isolation executes every exact allowed command within both limits and proves
  a disallowed negative control was never spawned.
- Release schemas parse, ratified policies are strict, and every ratified Cog
  has a committed lockfile.
- Action syntax passes, KMS signing has no digest flag, no private-key auth is
  present, and production workflows contain no staging signing configuration.
- A real GCP staging run and website-staging ingestion remain mandatory before
  any production signing ADR is proposed.

## Consequences

- A website release can be verified offline from exact bytes and a small
  public registry; neither GCP nor Sigstore availability is required at
  deployment time.
- Artifact transparency and release authorization remain independently
  reviewable instead of being conflated.
- Cloud KMS contains the private signing key; the repository, Actions logs,
  artifacts, and website receive only signatures and public material.
- Adding a release-eligible Cog now requires deliberate policy and lockfile
  review. That is additional work and an intentional release boundary.
- Production release signing remains blocked pending staging evidence and a
  separate production authority decision.

## Alternatives considered

- **Use only Sigstore keyless evidence** — rejected because its ECDSA artifact
  statement is not the website's Ed25519 full-release statement.
- **Store an Ed25519 private key as a GitHub secret** — rejected because
  long-lived exportable signing material is unnecessary.
- **Sign a digest or partial evidence document** — rejected because it does
  not authorize the exact record that is deployed.
- **Let the website reconstruct declarations after signing** — rejected
  because downstream reconstruction can change signed identity by omission.
- **Enable production signing in the same change** — rejected until isolated
  GCP staging and website ingestion evidence exist.

## Later decision: ADR-155

ADR-155 makes `runtimeIntegrations` a required part of the signed statement:
the exact normalized ADR-153 manifest, its canonical-byte digest, and a static
website bundle digest that is required only for `static-build`. The canonical
serializer now admits bounded JavaScript-safe integers needed by the manifest
while continuing to reject floats and unsafe integers. Production publication
remains frozen.

## Primary references

- [Cloud KMS key algorithms](https://cloud.google.com/kms/docs/algorithms)
- [Cloud KMS creating and validating signatures](https://cloud.google.com/kms/docs/create-validate-signatures)
- [Cloud KMS asymmetricSign API](https://cloud.google.com/kms/docs/reference/rest/v1/projects.locations.keyRings.cryptoKeys.cryptoKeyVersions/asymmetricSign)
- [Google GitHub Actions authentication](https://github.com/google-github-actions/auth)
- [Cosign keyless blob signing](https://docs.sigstore.dev/cosign/signing/signing_with_blobs/)
