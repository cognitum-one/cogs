# ADR-155: Bind runtime integrations into the signed Cog release

**Status**: Accepted
**Date**: 2026-07-29
**Related**: ADR-020 (build/publish), ADR-153 (optional runtime integrations),
ADR-154 (canonical Ed25519 release records), website ADR-113 (release
evidence), website ADR-123 (runtime reconciliation)

## Context

ADR-153 made every integration manifest canonical and content-addressed.
ADR-154 signed the complete deployment release record. The two controls were
not joined: the signed record did not contain the normalized integration
manifest or its digest, and the builder accepted no integration sidecar.

That left a substitution gap. A valid binary release and a valid integration
manifest could be combined after signing without invalidating the release
signature. A static website bundle was similarly uploaded beside a release
without its digest being part of the operator policy or signed identity.
Directory-wide workflow discovery made the boundary broader than the exact
files produced by the emitters.

The release statement also rejected all numbers. Enabled integration manifests
necessarily contain bounded integer ports, health intervals, timeouts, expiry,
and rate limits. Those integers have identical JSON representations in Python
and JavaScript, but floats and integers outside JavaScript's safe range do not.

## Decision

### 1. One signed `runtimeIntegrations` value is authoritative

The operator release policy, unsigned release, signed release, and final build
evidence carry the same exact object:

```json
{
  "runtimeIntegrations": {
    "manifest": {
      "schemaVersion": "cognitum.cog.integrations.v1",
      "cog": { "id": "anomaly-detect", "version": "1.2.0" },
      "integrations": {
        "website": { "enabled": false },
        "tailscale": { "enabled": false },
        "webMcp": { "enabled": false }
      }
    },
    "manifestDigest": "sha256:<canonical-manifest-bytes>",
    "staticWebsiteBundleDigest": null
  }
}
```

`manifest` is the exact normalized ADR-153 projection. `manifestDigest` is
SHA-256 over sorted, compact UTF-8 JSON with exactly one final newline. The
Cog id and version must equal the release identity. Unknown, omitted,
misplaced, or non-normalized fields fail.

`staticWebsiteBundleDigest` is always present:

- it is a required SHA-256 digest when the signed manifest enables a
  `static-build` website;
- it is exactly `null` for a disabled website and for a digest-pinned OCI
  website.

This conditional is enforced independently by the Python policy validator and
the operator-policy JSON Schema. OCI identity remains in the exact manifest;
the cogs workflow does not push an OCI image.

### 2. The builder accepts exact files, not directories or patterns

`cog_release_provenance.py prepare` requires
`--integration-manifest <exact-file>`. It rejects a missing path, directory,
symlink, oversized document, alternate JSON serialization, renamed file,
manifest/policy mismatch, digest mismatch, or release identity mismatch.

When a static website is enabled, prepare additionally requires
`--static-website-bundle <exact-file>`. The bundle must be a regular file with
the exact content-addressed filename and the digest ratified in policy. The
argument is rejected when the manifest does not require it.

The policy decision records `I1 runtime-integrations` and, conditionally,
`I2 static-website-bundle`. Finalization compares the complete
`runtimeIntegrations` object in measured build evidence with the signed
release. Altering either binding changes the canonical payload and invalidates
Ed25519 verification.

### 3. Cross-runtime canonical JSON admits safe integers only

Canonical release values may contain integers from
`-(2^53 - 1)` through `2^53 - 1`. Booleans remain distinct from integers.
Floats, non-finite values, larger integers, and every other numeric form are
rejected. Integration semantics impose much smaller field-specific bounds.

The committed public fixture is verified by both Python/OpenSSL and Node's
Ed25519 implementation. It contains a signed release, detached signature, and
public key registry only. No fixture private key is stored.

### 4. Workflows resolve exact outputs and production stays frozen

The staging, production, and batch build workflows resolve exactly one
manifest/checksum and, conditionally, one website bundle/checksum. They
recompute digests, require exact content-addressed filenames and checksum
contents, reject extra sidecars, and never select integration or website
release inputs with a directory-wide `find`.

Staging passes those exact paths into the complete release builder and uploads
only those exact sidecars. It remains manual, protected by `cogs-staging`, WIF
only, and the only Ed25519 signing lane.

Production and batch production upload paths fail closed before GCP
authentication or GCS upload. Production's pre-cloud validation still creates
GitHub/Sigstore keyless signatures and build attestations; those public
evidence records grant no GCP or production publication authority. Enabling
production requires a separate ADR, production Ed25519 trust root, reviewed
WIF/KMS authority, and staging ingestion evidence.

No source `cog.toml` enables website, Tailscale attachment, or web MCP in this
decision. `anomaly-detect` ratifies the canonical all-disabled manifest and a
null static website digest.

### 5. Workflow dependencies are immutable

All GitHub Actions are pinned to reviewed commit SHAs. The reusable
organization security workflow executes from immutable merged caller commit
`62489f1606ce871af9c0405dd9e1cb6f886b15cc`. Its reviewed internal policy
anchor is `647765efc5152db840e29104f68a28340ca232f9`; both commits are ancestors
of merged organization main commit
`5eb92ac7c8a8b72814663ab58d9241dd8e0510ae`.
Every repository checkout disables persisted workflow credentials. CI has
explicit read-only contents permission, so untrusted pull-request build code
cannot inherit a write-capable repository token.

Production's floating Syft `main` installer is replaced by Syft 1.44.0 archive
checksum
`0e91737aee2b5baf1d255b959630194a302335d848ff97bb07921eb6205b5f5a`.
`cargo-audit` is exactly 0.22.2 and installs with `--locked`; there is no
unlocked fallback.

### 6. Dependency scan remediation is exact and fail-closed

The final organization policy runs OSV-Scanner 2.2.4 from the immutable
Linux AMD64 binary digest
`7702cd1e5d9f5059dd9570f4ad967f27d3c5f5391b371ec937b384c238177f55`
with recursive, no-ignore, all-vulnerability traversal. The scan after this
decision's dependency remediation reports five residual advisories, all with
no fixed version: RSA timing advisory `RUSTSEC-2023-0071` at CVSS 5.9 and the
unmaintained-crate notices for `number_prefix`, `paste`, `proc-macro-error`,
and `proc-macro-error2`. The immutable policy gate therefore reports no
unreviewed High/Critical fixable vulnerability. These residual advisories
remain visible; they are not ignored or granted an exception.

Fixable findings were removed by committed lock resolution rather than scanner
suppression. The root graph now uses SQLx 0.8.6, `anyhow` 1.0.103,
`crossbeam-epoch` 0.9.20, `memmap2` 0.9.11, rand 0.8.6/0.9.3, and RSA 0.9.10.
AgentVM uses Wiremock 0.6.5, removing its obsolete `http-types` and rand 0.7
chain. The affected benchmark and FXNN locks carry the same independently
resolved patched checksums.

FIXEL's old ESLint 8 dependency graph could not consume a patched
`brace-expansion` without an invalid lock. It now uses ESLint 10.8.0,
`@eslint/js` 10.0.1, typescript-eslint 8.65.0, Minimatch 10.2.6, and
`brace-expansion` 5.0.8, with a flat TypeScript configuration and the exact
Node range required by ESLint 10. `npm ci`, audit, lint, test, and build all
pass without `--force`, an override, or an advisory suppression.

Two legacy benchmark manifests still reference absent local path crates:
`benchmarks` expects `cognitum-sim/crates/newport-raceway`, and
`benchmarks/stress-tests` expects `newport-sim/crates/newport-core`.
Their lock-only security updates copy exact version, checksum, and dependency
stanzas from Cargo-resolved locks in this repository; OSV scans them
successfully. Compiling those two legacy packages remains blocked until a
separate source-ownership decision repairs or retires the missing path
dependencies.

Validation on the final working tree passed the root locked check and 216
library tests with one explicitly ignored simulator test; locked all-target
checks for the AgentVM workspace, FXNN, and the v0 appliance benchmark; all 15
release-provenance policy tests; and the organization policy's 24 OSV-gate,
13 scanner-wrapper, and 38 static-runtime-receipt tests. Existing compiler
warnings remain non-fatal and are outside this dependency-only remediation.

## Website consumer requirement

Before a staging release can be ingested, `cognitum-one/website` must:

1. add `runtimeIntegrations` to its `CogRelease` type and release-registry
   schema as a required signed field;
2. recompute `manifestDigest` from the exact canonical manifest and enforce
   the static-bundle conditional independently of the cogs schema;
3. allow only safe integers in its canonical release serializer;
4. preserve `runtimeIntegrations` from the signed release/evidence instead of
   reconstructing it from an unsigned sidecar or local declaration;
5. make reconciliation consume only that signed manifest and require the
   signed static bundle digest before static hosting; and
6. add omission, substitution, bundle-tamper, and signature-tamper tests using
   the public fixture contract.

Until those changes and a real staging ingestion are validated, no Cog release
is production-deployable through this integration path.

## Required validation

- every catalog `cog.toml` normalizes and all real integrations remain off;
- a dependency-free local gate resolves every release-schema reference and
  validates the ratified policy plus public release/trust fixtures;
- the ratified policy digest equals an independently emitted manifest;
- mutation tests reject missing, broad, renamed, symlinked, oversized,
  alternate-byte, tampered, and injected integration sidecars;
- static website tests prove the bundle is required, exact, and signed;
- Python and Node produce identical canonical bytes at both JavaScript-safe
  integer boundaries and verify the committed public Ed25519 fixture;
- action policy tests kill unpinned-action, omitted-manifest, broad-directory,
  unfrozen-production, restored-cloud-authority, and floating-tool mutations;
- action syntax passes with the pinned Actionlint binary;
- focused Cargo checks continue to use committed locks; and
- a real GCP staging run plus website-staging ingestion is still required
  before any production signing or integration reconciliation authority.

## Consequences

- A release signature now authorizes the binary and exact optional runtime
  interfaces as one identity.
- Static website bytes cannot be exchanged independently of the signature.
- Source policy must change deliberately when a manifest or static bundle
  changes.
- Production publication is intentionally unavailable until separately
  ratified.
- The website contract must be updated before it can ingest the new field.

## Alternatives considered

- **Sign only the manifest digest** — rejected because consumers also need the
  exact normalized desired state without trusting an unsigned lookup.
- **Upload everything under an output directory** — rejected because an
  injected or stale sidecar would become an ambiguous release input.
- **Infer the website bundle after signing** — rejected because a filename is
  not operator authorization.
- **Permit optional `runtimeIntegrations`** — rejected because omission
  recreates the substitution gap.
- **Enable production KMS signing here** — rejected because staging ingestion
  evidence and a separate authority decision do not yet exist.
