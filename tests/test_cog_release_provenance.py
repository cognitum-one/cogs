"""Security and cross-runtime tests for detached Cog release provenance."""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from cog_release_provenance import (  # noqa: E402
    admission,
    finalize,
    finalize_release_validity,
    finalize_withdrawal,
    prepare_release_validity,
    prepare_withdrawal,
    registry,
    verify,
    verify_admitted_release,
    verify_release_validity,
    verify_trust_registry,
    verify_withdrawal,
    withdrawal_projection_decision,
)
from cog_release_builder import prepare  # noqa: E402
from cog_integrations import (  # noqa: E402
    MAX_MANIFEST_BYTES,
    ManifestValidationError,
    canonical_bytes,
    emit_manifest,
    load_canonical_manifest,
    normalize_manifest,
)
from cog_release_provenance_lib import (  # noqa: E402
    EVIDENCE_LOCATIONS_SCHEMA,
    ReleaseError,
    canonical_payload,
    canonical_withdrawal_payload,
    payload_digest,
    validate_evidence_locations,
    validate_release,
    validate_policy,
    validate_runtime_cache_policy,
    runtime_cache_decision,
    withdrawal_payload_digest,
)
from cog_trust_registry import (  # noqa: E402
    ROOT_IDENTITIES,
    bootstrap_digest,
    canonical_registry_payload,
    registry_payload_digest,
    validate_bootstrap,
    validate_registry,
)
import cog_release_schema_check as schema_gate  # noqa: E402

POLICY = ROOT / "src" / "cogs" / "anomaly-detect" / "release-policy.json"
PUBLIC_FIXTURE = ROOT / "tests" / "fixtures" / "cog-release"
WORKFLOW_DIR = ROOT / ".github" / "workflows"
BUILDER = "github-actions://cognitum-one/cogs"
WORKFLOW = (
    "cognitum-one/cogs/.github/workflows/publish-cog-staging.yml"
    "@refs/heads/codex/cog-optional-web-tailscale-mcp"
)
KEY_ID = "gcp-kms:cogs-staging-release-2026-01"
KMS_KEY_VERSION = (
    "projects/cognitum-20260110/locations/us-central1/keyRings/"
    "cog-release-stg/cryptoKeys/release-ed25519/cryptoKeyVersions/1"
)
GITHUB_OWNER_ID = "256911919"
GITHUB_REPOSITORY_ID = "1211713542"
GITHUB_WORKFLOW_ID = "322710413"
WITHDRAWAL_KEY_ID = "gcp-kms:cogs-staging-withdrawal-2026-01"
WITHDRAWAL_KMS_KEY_VERSION = (
    "projects/cognitum-20260110/locations/us-central1/keyRings/"
    "cog-withdrawal-stg/cryptoKeys/withdrawal-ed25519/cryptoKeyVersions/1"
)
WITHDRAWAL_WORKFLOW = (
    "cognitum-one/cogs/.github/workflows/withdraw-cog-staging.yml"
    "@refs/heads/main"
)
WITHDRAWAL_WORKFLOW_ID = "422710413"
WITHDRAWAL_WORKFLOW_SHA = "b" * 40


def run(*command: str) -> None:
    subprocess.run(command, check=True, capture_output=True)


class CogReleaseProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.artifact = self.directory / "cog-anomaly-detect-arm64"
        self.bundle = self.directory / "artifact.sigstore.json"
        self.lock = self.directory / "Cargo.lock"
        self.sbom = self.directory / "sbom.cdx.json"
        self.vulnerability = self.directory / "vuln-scan.json"
        self.provenance = self.directory / "provenance.sigstore.json"
        self.isolation = self.directory / "isolation-runs.json"
        self.output = self.directory / "release"
        self.artifact.write_bytes(b"test artifact")
        self.bundle.write_text(
            json.dumps(
                {
                    "mediaType": (
                        "application/vnd.dev.sigstore.bundle+json;version=0.3"
                    ),
                    "verificationMaterial": {
                        "tlogEntries": [
                            {
                                "logIndex": "1",
                                "logId": {"keyId": "fixture-log"},
                                "inclusionPromise": {
                                    "signedEntryTimestamp": "fixture-set"
                                },
                            }
                        ]
                    },
                }
            )
        )
        self.lock.write_text("# deterministic lock")
        self.sbom.write_text('{"bomFormat":"CycloneDX"}')
        self.vulnerability.write_text('{"vulnerabilities":{"list":[]},"warnings":{}}')
        self.provenance.write_text('{"dsseEnvelope":{}}')
        self.isolation.write_text(
            json.dumps(
                {
                    "schema": "cognitum.cog.isolation-evidence.v1",
                    "cogId": "anomaly-detect",
                    "runOnTarget": False,
                    "passed": True,
                    "policy": {
                        "allowed_commands": ["--once"],
                        "max_runtime_secs": 15,
                        "output_limit_bytes": 65536,
                    },
                    "runs": [
                        {
                            "command": "--once",
                            "exit": 0,
                            "evidence": {
                                "within_runtime_limit": True,
                                "within_output_limit": True,
                            },
                        },
                        {
                            "command": "--definitely-not-allowed",
                            "exit": 4,
                            "refused": True,
                            "spawned": False,
                            "evidence": None,
                        },
                    ],
                }
            )
        )
        self.integration_manifest, _, self.integration_checksum = emit_manifest(
            ROOT / "src" / "cogs" / "anomaly-detect" / "cog.toml",
            self.directory / "integrations",
        )
        self.prepare_args = argparse.Namespace(
            policy=POLICY,
            artifact=self.artifact,
            integration_manifest=self.integration_manifest,
            static_website_bundle=None,
            sigstore_bundle=self.bundle,
            dependency_lock=self.lock,
            sbom=self.sbom,
            vulnerability_scan=self.vulnerability,
            provenance=self.provenance,
            isolation=self.isolation,
            output_dir=self.output,
            arch="aarch64",
            version="1.2.0",
            source_commit="a" * 40,
            key_id=KEY_ID,
            builder_identity=BUILDER,
            build_workflow=WORKFLOW,
            built_at="2026-07-29T12:00:00Z",
            repository="cognitum-one/cogs",
            sigstore_tool_version="cosign@v3.1.2",
        )
        prepare(self.prepare_args)
        self.private_key = self.directory / "private.pem"
        self.public_key = self.directory / "public.pem"
        self.signature_bin = self.directory / "signature.bin"
        self.signature_b64 = self.directory / "signature.b64"
        run(
            "openssl",
            "genpkey",
            "-algorithm",
            "ED25519",
            "-out",
            str(self.private_key),
        )
        run(
            "openssl",
            "pkey",
            "-in",
            str(self.private_key),
            "-pubout",
            "-out",
            str(self.public_key),
        )
        run(
            "openssl",
            "pkeyutl",
            "-sign",
            "-inkey",
            str(self.private_key),
            "-rawin",
            "-in",
            str(self.output / "release-statement.json"),
            "-out",
            str(self.signature_bin),
        )
        self.signature_b64.write_text(
            base64.b64encode(self.signature_bin.read_bytes()).decode()
        )
        self.registry = self.output / "release-trust-registry.json"
        self.registry_args = argparse.Namespace(
            public_key=self.public_key,
            key_id=KEY_ID,
            kms_key_version=KMS_KEY_VERSION,
            kms_algorithm="EC_SIGN_ED25519",
            protection_level="SOFTWARE",
            purpose=["release"],
            builder_identity=BUILDER,
            build_workflow=WORKFLOW,
            workflow_sha="a" * 40,
            sequence=1,
            previous_registry=None,
            existing_registry=None,
            issued_at="2026-07-29T12:00:00Z",
            not_before="2026-07-29T12:00:00Z",
            expires_at="2026-10-27T12:00:00Z",
            key_not_before="2026-07-29T00:00:00Z",
            key_expires_at="2027-07-29T00:00:00Z",
            github_owner_id=GITHUB_OWNER_ID,
            github_repository_id=GITHUB_REPOSITORY_ID,
            github_workflow_id=[GITHUB_WORKFLOW_ID],
            output=self.registry,
        )
        registry(self.registry_args)
        self.evidence = self.output / "release-evidence.json"
        self.release = self.output / "signed-release.json"
        finalize(
            argparse.Namespace(
                unsigned_release=self.output / "unsigned-release.json",
                build_evidence=self.output / "build-evidence.json",
                signature_file=self.signature_b64,
                output=self.evidence,
                signed_release_output=self.release,
            )
        )
        prepare_release_validity(
            argparse.Namespace(
                release=self.release,
                registry=self.registry,
                issued_at="2026-07-29T12:01:00Z",
                signed_at="2026-07-29T12:01:00Z",
                not_before="2026-07-29T12:01:00Z",
                expires_at="2026-08-28T12:01:00Z",
                output_dir=self.output,
            )
        )
        self.validity_signature_bin = self.output / "validity-signature.bin"
        self.validity_signature_b64 = self.output / "validity-signature.b64"
        run(
            "openssl",
            "pkeyutl",
            "-sign",
            "-inkey",
            str(self.private_key),
            "-rawin",
            "-in",
            str(self.output / "release-validity-statement.json"),
            "-out",
            str(self.validity_signature_bin),
        )
        self.validity_signature_b64.write_text(
            base64.b64encode(self.validity_signature_bin.read_bytes()).decode()
        )
        self.validity = self.output / "signed-release-validity.json"
        finalize_release_validity(
            argparse.Namespace(
                release=self.release,
                registry=self.registry,
                unsigned_validity=self.output / "unsigned-release-validity.json",
                signature_file=self.validity_signature_b64,
                output=self.validity,
            )
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _generate_ed25519_key(
        self, name: str
    ) -> tuple[Path, Path, str]:
        directory = self.directory / name
        directory.mkdir(parents=True, exist_ok=True)
        private_key = directory / "private.pem"
        public_key = directory / "public.pem"
        run(
            "openssl",
            "genpkey",
            "-algorithm",
            "ED25519",
            "-out",
            str(private_key),
        )
        run(
            "openssl",
            "pkey",
            "-in",
            str(private_key),
            "-pubout",
            "-out",
            str(public_key),
        )
        der = subprocess.run(
            [
                "openssl",
                "pkey",
                "-pubin",
                "-in",
                str(public_key),
                "-outform",
                "DER",
            ],
            check=True,
            capture_output=True,
        ).stdout
        return (
            private_key,
            public_key,
            "sha256:" + hashlib.sha256(der).hexdigest(),
        )

    def _sign_registry(
        self,
        registry_value: dict,
        signers: list[tuple[str, Path]],
        *,
        name: str,
    ) -> dict:
        unsigned = copy.deepcopy(registry_value)
        unsigned.pop("signatures", None)
        payload_path = self.directory / f"{name}-registry-statement.json"
        payload_path.write_bytes(canonical_registry_payload(unsigned))
        signatures = []
        for index, (key_id, private_key) in enumerate(signers):
            signature_path = self.directory / f"{name}-root-{index}.sig"
            run(
                "openssl",
                "pkeyutl",
                "-sign",
                "-inkey",
                str(private_key),
                "-rawin",
                "-in",
                str(payload_path),
                "-out",
                str(signature_path),
            )
            signatures.append(
                {
                    "schema": "cognitum.cog.trust-registry.v3",
                    "algorithm": "ed25519",
                    "keyId": key_id,
                    "payloadDigest": registry_payload_digest(unsigned),
                    "signature": base64.urlsafe_b64encode(
                        signature_path.read_bytes()
                    )
                    .decode("ascii")
                    .rstrip("="),
                }
            )
        unsigned["signatures"] = signatures
        return unsigned

    def _root_authority_fixture(
        self,
    ) -> tuple[dict, list[tuple[str, Path]]]:
        roots = []
        signers = []
        for index, (role, key_id) in enumerate(ROOT_IDENTITIES.items(), start=1):
            private_key, public_key, fingerprint = self._generate_ed25519_key(
                f"trust-root-{index}"
            )
            roots.append(
                {
                    "role": role,
                    "keyId": key_id,
                    "signingResource": (
                        "projects/cognitum-20260110/locations/us-central1/"
                        f"keyRings/cog-trust-root-{index}/cryptoKeys/"
                        f"root-{index}-ed25519/cryptoKeyVersions/1"
                    ),
                    "algorithm": "ed25519",
                    "publicKeyFingerprint": fingerprint,
                    "publicKeyPem": public_key.read_text(),
                }
            )
            signers.append((key_id, private_key))
        bootstrap = {
            "schema": "cognitum.cog.trust-bootstrap.v1",
            "threshold": 2,
            "roots": roots,
        }
        validate_bootstrap(bootstrap)
        return bootstrap, signers

    def test_ed25519_round_trip_preserves_sigstore_evidence(self) -> None:
        verify(argparse.Namespace(release=self.release, registry=self.registry))
        evidence = json.loads(self.evidence.read_text())
        envelope = evidence["releaseSignature"]
        self.assertEqual(envelope["algorithm"], "ed25519")
        self.assertEqual(len(envelope["signature"]), 86)
        self.assertEqual(evidence["signatureAlgorithm"], "ecdsa-p256")
        self.assertEqual(evidence["signatureFormat"], "sigstore-bundle")
        self.assertEqual(
            evidence["releaseRecord"], json.loads(self.release.read_text())
        )
        self.assertNotIn("PRIVATE KEY", self.evidence.read_text())
        self.assertNotIn("PRIVATE KEY", self.registry.read_text())

    def test_trust_v3_candidate_binds_kms_purpose_workflow_and_numeric_ids(
        self,
    ) -> None:
        trust = json.loads(self.registry.read_text())
        key = trust["releases"][0]
        self.assertEqual(trust["schema"], "cognitum.cog.trust-registry.v3")
        self.assertEqual(trust["sequence"], 1)
        self.assertEqual(trust["previousRegistryDigest"], "GENESIS")
        self.assertEqual(trust["withdrawals"], [])
        self.assertEqual(key["kmsAlgorithm"], "EC_SIGN_ED25519")
        self.assertEqual(key["kmsKeyVersion"], KMS_KEY_VERSION)
        self.assertEqual(key["protectionLevel"], "software")
        self.assertEqual(key["purpose"], "release")
        self.assertEqual(key["workflowSha"], "a" * 40)
        self.assertEqual(
            key["github"],
            {
                "ownerId": GITHUB_OWNER_ID,
                "repositoryId": GITHUB_REPOSITORY_ID,
                "workflowIds": [GITHUB_WORKFLOW_ID],
            },
        )
        self.assertRegex(key["publicKeyFingerprint"], r"^sha256:[a-f0-9]{64}$")

        for name, mutation in (
            ("hsm", {"protectionLevel": "hsm"}),
            ("fingerprint", {"publicKeyFingerprint": "sha256:" + "0" * 64}),
            (
                "repository-name",
                {
                    "github": {
                        **key["github"],
                        "repositoryId": "cognitum-one/cogs",
                    }
                },
            ),
            ("wrong-purpose", {"purpose": "withdrawal"}),
        ):
            with self.subTest(name=name):
                mutated = copy.deepcopy(trust)
                mutated["releases"][0].update(mutation)
                path = self.output / f"bad-trust-{name}.json"
                path.write_text(json.dumps(mutated))
                with self.assertRaises(ReleaseError):
                    verify(argparse.Namespace(release=self.release, registry=path))

    def test_quorum_rooted_registry_and_hash_chain_fail_closed(self) -> None:
        bootstrap, signers = self._root_authority_fixture()
        bootstrap_path = self.output / "trust-bootstrap.json"
        bootstrap_path.write_text(json.dumps(bootstrap))
        genesis = self._sign_registry(
            json.loads(self.registry.read_text()),
            signers[:2],
            name="genesis",
        )
        genesis_path = self.output / "signed-trust-registry-genesis.json"
        genesis_path.write_text(json.dumps(genesis))

        def verify_registry(
            registry_path: Path,
            *,
            expected_registry: str,
            expected_bootstrap: str | None = None,
            minimum_sequence: int = 1,
            previous_registry: Path | None = None,
            checked_at: str = "2026-07-29T12:30:00Z",
            selected_bootstrap: Path = bootstrap_path,
        ) -> None:
            verify_trust_registry(
                argparse.Namespace(
                    bootstrap=selected_bootstrap,
                    registry=registry_path,
                    expected_bootstrap_digest=(
                        expected_bootstrap or bootstrap_digest(bootstrap)
                    ),
                    expected_registry_digest=expected_registry,
                    minimum_sequence=minimum_sequence,
                    previous_registry=previous_registry,
                    checked_at=checked_at,
                )
            )

        verify_registry(
            genesis_path,
            expected_registry=registry_payload_digest(genesis),
        )
        verify_admitted_release(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                bootstrap=bootstrap_path,
                registry=genesis_path,
                expected_bootstrap_digest=bootstrap_digest(bootstrap),
                expected_registry_digest=registry_payload_digest(genesis),
                minimum_sequence=1,
                previous_registry=None,
                checked_at="2026-07-29T12:30:00Z",
            )
        )

        sequence_two = copy.deepcopy(genesis)
        sequence_two.pop("signatures")
        sequence_two.update(
            {
                "sequence": 2,
                "previousRegistryDigest": registry_payload_digest(genesis),
                "issuedAt": "2026-07-29T13:00:00Z",
                "notBefore": "2026-07-29T13:00:00Z",
                "expiresAt": "2026-10-27T13:00:00Z",
            }
        )
        sequence_two = self._sign_registry(
            sequence_two,
            signers[1:],
            name="sequence-two",
        )
        sequence_two_path = self.output / "signed-trust-registry-2.json"
        sequence_two_path.write_text(json.dumps(sequence_two))
        verify_registry(
            sequence_two_path,
            expected_registry=registry_payload_digest(sequence_two),
            minimum_sequence=2,
            previous_registry=genesis_path,
            checked_at="2026-07-29T13:01:00Z",
        )

        one_signature = copy.deepcopy(genesis)
        one_signature["signatures"] = one_signature["signatures"][:1]
        duplicate_signature = copy.deepcopy(genesis)
        duplicate_signature["signatures"][1]["keyId"] = (
            duplicate_signature["signatures"][0]["keyId"]
        )
        unknown_root = copy.deepcopy(genesis)
        unknown_root["signatures"][1]["keyId"] = "unknown/cog-trust-root"
        tampered_payload = copy.deepcopy(genesis)
        tampered_payload["releases"][0]["workflowSha"] = "f" * 40
        for name, value in (
            ("one-signature", one_signature),
            ("duplicate-signature", duplicate_signature),
            ("unknown-root", unknown_root),
            ("signed-payload-tamper", tampered_payload),
        ):
            with self.subTest(adversarial=name):
                path = self.output / f"bad-registry-{name}.json"
                path.write_text(json.dumps(value))
                with self.assertRaises(ReleaseError):
                    verify_registry(
                        path,
                        expected_registry=registry_payload_digest(value),
                    )

        with self.subTest(adversarial="source-pin-substitution"):
            with self.assertRaises(ReleaseError):
                verify_registry(
                    genesis_path,
                    expected_registry=registry_payload_digest(genesis),
                    expected_bootstrap="sha256:" + "0" * 64,
                )

        mutated_bootstrap = copy.deepcopy(bootstrap)
        mutated_bootstrap["roots"][0]["publicKeyFingerprint"] = (
            "sha256:" + "0" * 64
        )
        mutated_bootstrap_path = self.output / "bad-bootstrap-fingerprint.json"
        mutated_bootstrap_path.write_text(json.dumps(mutated_bootstrap))
        with self.subTest(adversarial="bootstrap-fingerprint-substitution"):
            with self.assertRaises(ReleaseError):
                verify_registry(
                    genesis_path,
                    expected_registry=registry_payload_digest(genesis),
                    expected_bootstrap=bootstrap_digest(mutated_bootstrap),
                    selected_bootstrap=mutated_bootstrap_path,
                )

        with self.subTest(adversarial="registry-rollback"):
            with self.assertRaises(ReleaseError):
                verify_registry(
                    genesis_path,
                    expected_registry=registry_payload_digest(genesis),
                    minimum_sequence=2,
                )

        with self.subTest(adversarial="expired-registry"):
            with self.assertRaises(ReleaseError):
                verify_registry(
                    genesis_path,
                    expected_registry=registry_payload_digest(genesis),
                    checked_at="2026-10-28T00:00:00Z",
                )

        for name, sequence, predecessor_digest in (
            ("broken-predecessor", 2, "sha256:" + "0" * 64),
            ("sequence-gap", 3, registry_payload_digest(genesis)),
        ):
            chained = copy.deepcopy(sequence_two)
            chained.pop("signatures")
            chained["sequence"] = sequence
            chained["previousRegistryDigest"] = predecessor_digest
            chained = self._sign_registry(
                chained,
                signers[:2],
                name=name,
            )
            path = self.output / f"bad-registry-{name}.json"
            path.write_text(json.dumps(chained))
            with self.subTest(adversarial=name):
                with self.assertRaises(ReleaseError):
                    verify_registry(
                        path,
                        expected_registry=registry_payload_digest(chained),
                        previous_registry=genesis_path,
                        checked_at="2026-07-29T13:01:00Z",
                    )

    def test_release_validity_time_bounds_and_signature_fail_closed(self) -> None:
        verify_release_validity(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                registry=self.registry,
                checked_at="2026-08-01T00:00:00Z",
            )
        )
        with self.assertRaises(ReleaseError):
            verify_release_validity(
                argparse.Namespace(
                    release=self.release,
                    validity=self.validity,
                    registry=self.registry,
                    checked_at="2026-08-28T12:06:00Z",
                )
            )

        tampered = json.loads(self.validity.read_text())
        tampered["expiresAt"] = "2026-08-27T12:01:00Z"
        tampered_path = self.output / "tampered-release-validity.json"
        tampered_path.write_text(json.dumps(tampered))
        with self.assertRaises(ReleaseError):
            verify_release_validity(
                argparse.Namespace(
                    release=self.release,
                    validity=tampered_path,
                    registry=self.registry,
                    checked_at="2026-08-01T00:00:00Z",
                )
            )

        future_revocation = json.loads(self.registry.read_text())
        future_revocation["releases"][0].update(
            {
                "status": "revoked",
                "revocation": {
                    "effectiveAt": "2026-07-29T13:00:00Z",
                    "reasonCode": "security.key",
                    "scope": "future-signatures",
                },
            }
        )
        future_revocation_path = self.output / "future-key-revocation.json"
        future_revocation_path.write_text(json.dumps(future_revocation))
        verify_release_validity(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                registry=future_revocation_path,
                checked_at="2026-08-01T00:00:00Z",
            )
        )
        with self.assertRaises(ReleaseError):
            verify(
                argparse.Namespace(
                    release=self.release,
                    registry=future_revocation_path,
                )
            )

        for name, effective_at, scope in (
            ("all-signatures", "2026-07-29T13:00:00Z", "all-signatures"),
            (
                "future-signatures-after-effective",
                "2026-07-29T12:00:00Z",
                "future-signatures",
            ),
        ):
            revoked = copy.deepcopy(future_revocation)
            revoked["releases"][0]["revocation"].update(
                {"effectiveAt": effective_at, "scope": scope}
            )
            revoked_path = self.output / f"revoked-{name}.json"
            revoked_path.write_text(json.dumps(revoked))
            with self.subTest(adversarial=name), self.assertRaises(ReleaseError):
                verify_release_validity(
                    argparse.Namespace(
                        release=self.release,
                        validity=self.validity,
                        registry=revoked_path,
                        checked_at="2026-08-01T00:00:00Z",
                    )
                )

        for name, times in (
            (
                "overlong-lifetime",
                {
                    "issued_at": "2026-07-29T12:01:00Z",
                    "signed_at": "2026-07-29T12:01:00Z",
                    "not_before": "2026-07-29T12:01:00Z",
                    "expires_at": "2026-08-29T12:01:00Z",
                },
            ),
            (
                "signed-before-issuance",
                {
                    "issued_at": "2026-07-29T12:01:00Z",
                    "signed_at": "2026-07-29T12:00:59Z",
                    "not_before": "2026-07-29T12:01:00Z",
                    "expires_at": "2026-08-28T12:01:00Z",
                },
            ),
            (
                "signed-after-skew",
                {
                    "issued_at": "2026-07-29T12:01:00Z",
                    "signed_at": "2026-07-29T12:06:01Z",
                    "not_before": "2026-07-29T12:01:00Z",
                    "expires_at": "2026-08-28T12:01:00Z",
                },
            ),
        ):
            with self.subTest(adversarial=name), self.assertRaises(ReleaseError):
                prepare_release_validity(
                    argparse.Namespace(
                        release=self.release,
                        registry=self.registry,
                        output_dir=self.output / name,
                        **times,
                    )
                )

    def test_runtime_cache_staleness_policy_is_exact_and_fail_closed(self) -> None:
        policy = json.loads(
            (ROOT / "config" / "cog-release-runtime-cache-policy.json").read_text()
        )
        validate_runtime_cache_policy(policy)
        self.assertEqual(
            runtime_cache_decision(
                policy,
                source="trustRegistry",
                fetched_at="2026-07-29T12:00:00Z",
                checked_at="2026-07-29T12:05:00Z",
                refresh_succeeded=False,
                operation="new-deployment",
            ),
            "fresh",
        )
        self.assertEqual(
            runtime_cache_decision(
                policy,
                source="trustRegistry",
                fetched_at="2026-07-29T12:00:00Z",
                checked_at="2026-07-29T12:05:01Z",
                refresh_succeeded=False,
                operation="new-deployment",
            ),
            "deny-dependency-unavailable",
        )
        self.assertEqual(
            runtime_cache_decision(
                policy,
                source="withdrawalProjection",
                fetched_at="2026-07-29T12:00:00Z",
                checked_at="2026-07-29T12:00:31Z",
                refresh_succeeded=False,
                operation="running-workload",
            ),
            "continue",
        )
        self.assertEqual(
            policy["onRefreshFailure"]["coldStart"],
            "zero-deployable",
        )
        for name, mutate in (
            (
                "long-registry-ttl",
                lambda value: value["sources"]["trustRegistry"].update(
                    {"ttlSeconds": 301}
                ),
            ),
            (
                "long-negative-withdrawal-ttl",
                lambda value: value.update(
                    {"negativeWithdrawalTtlSeconds": 31}
                ),
            ),
            (
                "fail-open-new-deploy",
                lambda value: value["onRefreshFailure"].update(
                    {"newDeploymentAfterTtl": "continue"}
                ),
            ),
        ):
            with self.subTest(adversarial=name):
                mutated = copy.deepcopy(policy)
                mutate(mutated)
                with self.assertRaises(ReleaseError):
                    validate_runtime_cache_policy(mutated)

    def test_sigstore_transparency_entry_is_mandatory(self) -> None:
        for name, entries in (
            ("missing-entry", []),
            (
                "empty-inclusion-promise",
                [
                    {
                        "logIndex": "1",
                        "logId": {"keyId": "fixture-log"},
                        "inclusionPromise": {},
                    }
                ],
            ),
        ):
            with self.subTest(adversarial=name):
                missing_tlog = (
                    self.directory / f"artifact-{name}.sigstore.json"
                )
                missing_tlog.write_text(
                    json.dumps(
                        {
                            "mediaType": (
                                "application/vnd.dev.sigstore.bundle+json;"
                                "version=0.3"
                            ),
                            "verificationMaterial": {"tlogEntries": entries},
                        }
                    )
                )
                args = argparse.Namespace(**vars(self.prepare_args))
                args.sigstore_bundle = missing_tlog
                args.output_dir = self.directory / f"{name}-release"
                with self.assertRaises(ReleaseError):
                    prepare(args)

    def test_evidence_matrix_is_complete_deterministic_and_owned(self) -> None:
        matrix = json.loads(
            (ROOT / "config" / "cog-release-evidence-matrix.v1.json").read_text()
        )
        self.assertEqual(
            set(matrix),
            {
                "schema",
                "adr",
                "sourceAuthority",
                "artifactTemplate",
                "requiredEnvelopeFields",
                "entries",
            },
        )
        self.assertEqual(
            matrix["schema"], "cognitum.cog.release-evidence-matrix.v1"
        )
        expected_ids = [f"EV-128-{index:02d}" for index in range(1, 15)]
        self.assertEqual(
            [entry["id"] for entry in matrix["entries"]],
            expected_ids,
        )
        required_entry_fields = {
            "id",
            "owner",
            "independentReviewer",
            "command",
            "expected",
            "artifact",
            "sourceStatus",
        }
        for entry in matrix["entries"]:
            self.assertEqual(set(entry), required_entry_fields)
            self.assertTrue(entry["owner"])
            self.assertTrue(entry["independentReviewer"])
            self.assertTrue(entry["command"])
            self.assertTrue(entry["expected"])
            self.assertEqual(
                entry["artifact"],
                f"evidence/adr-156/<run-id>/{entry['id']}.json",
            )
        envelope_fields = set(matrix["requiredEnvelopeFields"])
        for field in (
            "startedAt",
            "finishedAt",
            "inputDigest",
            "stdoutDigest",
            "stderrDigest",
            "sourceSha",
            "previousReceiptDigest",
        ):
            self.assertIn(field, envelope_fields)

    def test_signed_withdrawal_round_trip_and_tampering_fail_closed(self) -> None:
        withdrawal_dir = self.directory / "withdrawal"
        withdrawal_private_key = withdrawal_dir / "private.pem"
        withdrawal_public_key = withdrawal_dir / "public.pem"
        withdrawal_dir.mkdir(parents=True)
        run(
            "openssl",
            "genpkey",
            "-algorithm",
            "ED25519",
            "-out",
            str(withdrawal_private_key),
        )
        run(
            "openssl",
            "pkey",
            "-in",
            str(withdrawal_private_key),
            "-pubout",
            "-out",
            str(withdrawal_public_key),
        )
        der = subprocess.run(
            [
                "openssl",
                "pkey",
                "-pubin",
                "-in",
                str(withdrawal_public_key),
                "-outform",
                "DER",
            ],
            check=True,
            capture_output=True,
        ).stdout
        trust = json.loads(self.registry.read_text())
        trust["withdrawals"].append(
            {
                "keyId": WITHDRAWAL_KEY_ID,
                "algorithm": "ed25519",
                "kmsAlgorithm": "EC_SIGN_ED25519",
                "kmsKeyVersion": WITHDRAWAL_KMS_KEY_VERSION,
                "protectionLevel": "software",
                "publicKeyFingerprint": (
                    "sha256:" + hashlib.sha256(der).hexdigest()
                ),
                "publicKeyPem": withdrawal_public_key.read_text(),
                "status": "active",
                "purpose": "withdrawal",
                "notBefore": "2026-07-29T00:00:00Z",
                "expiresAt": "2027-07-29T00:00:00Z",
                "builderIdentities": [f"{BUILDER}:withdrawal"],
                "buildWorkflows": [WITHDRAWAL_WORKFLOW],
                "workflowSha": WITHDRAWAL_WORKFLOW_SHA,
                "github": {
                    "ownerId": GITHUB_OWNER_ID,
                    "repositoryId": GITHUB_REPOSITORY_ID,
                    "workflowIds": [WITHDRAWAL_WORKFLOW_ID],
                },
                "revocation": None,
            }
        )
        validate_registry(trust, signed=False)
        combined_registry = withdrawal_dir / "trust-registry.json"
        combined_registry.write_text(json.dumps(trust))
        withdrawal_arguments = {
            "release": self.release,
            "validity": self.validity,
            "registry": combined_registry,
            "action": "withdrawn",
            "reason_code": "security.policy",
            "issued_at": "2026-07-29T20:00:00Z",
            "effective_at": "2026-07-29T20:00:00Z",
            "key_id": WITHDRAWAL_KEY_ID,
            "issuer_identity": f"{BUILDER}:withdrawal",
            "issuer_workflow": WITHDRAWAL_WORKFLOW,
            "github_owner_id": GITHUB_OWNER_ID,
            "github_repository_id": GITHUB_REPOSITORY_ID,
            "github_workflow_id": WITHDRAWAL_WORKFLOW_ID,
            "workflow_sha": WITHDRAWAL_WORKFLOW_SHA,
            "output_dir": withdrawal_dir,
        }
        for name, mutation in (
            ("release-purpose-key", {"key_id": KEY_ID}),
            (
                "effective-before-release",
                {"effective_at": "2026-07-29T12:00:59Z"},
            ),
            (
                "effective-after-issuance",
                {"effective_at": "2026-07-29T20:00:01Z"},
            ),
        ):
            with self.subTest(adversarial=name), self.assertRaises(ReleaseError):
                prepare_withdrawal(
                    argparse.Namespace(**(withdrawal_arguments | mutation))
                )
        release_as_withdrawal = copy.deepcopy(trust)
        misplaced_release = release_as_withdrawal["releases"].pop()
        misplaced_release["purpose"] = "withdrawal"
        release_as_withdrawal["withdrawals"].append(misplaced_release)
        release_as_withdrawal_path = withdrawal_dir / "release-as-withdrawal.json"
        release_as_withdrawal_path.write_text(json.dumps(release_as_withdrawal))
        with self.assertRaises(ReleaseError):
            verify(
                argparse.Namespace(
                    release=self.release,
                    registry=release_as_withdrawal_path,
                )
            )

        prepare_withdrawal(argparse.Namespace(**withdrawal_arguments))
        withdrawal_signature = withdrawal_dir / "withdrawal-signature.bin"
        withdrawal_signature_b64 = withdrawal_dir / "withdrawal-signature.b64"
        run(
            "openssl",
            "pkeyutl",
            "-sign",
            "-inkey",
            str(withdrawal_private_key),
            "-rawin",
            "-in",
            str(withdrawal_dir / "withdrawal-statement.json"),
            "-out",
            str(withdrawal_signature),
        )
        withdrawal_signature_b64.write_text(
            base64.b64encode(withdrawal_signature.read_bytes()).decode()
        )
        evidence = withdrawal_dir / "release-withdrawal.json"
        signed = withdrawal_dir / "signed-withdrawal.json"
        finalize_withdrawal(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                registry=combined_registry,
                unsigned_withdrawal=withdrawal_dir / "unsigned-withdrawal.json",
                signature_file=withdrawal_signature_b64,
                key_id=WITHDRAWAL_KEY_ID,
                output=evidence,
                signed_withdrawal_output=signed,
            )
        )
        verify_withdrawal(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                withdrawal=signed,
                registry=combined_registry,
            )
        )
        record = json.loads(signed.read_text())
        wrapper = json.loads(evidence.read_text())
        self.assertEqual(
            canonical_withdrawal_payload(record),
            (withdrawal_dir / "withdrawal-statement.json").read_bytes(),
        )
        self.assertEqual(
            record["detachedSignature"]["payloadDigest"],
            withdrawal_payload_digest(record),
        )
        self.assertEqual(wrapper["withdrawalRecord"], record)
        self.assertEqual(
            wrapper["withdrawalSignature"], record["detachedSignature"]
        )
        node = subprocess.run(
            [
                "node",
                "-e",
                """
const crypto=require('crypto');
const fs=require('fs');
const withdrawal=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));
const trust=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
const envelope=withdrawal.detachedSignature;
const key=trust.withdrawals.find(k=>k.keyId===envelope.keyId).publicKeyPem;
delete withdrawal.detachedSignature;
delete withdrawal.seededAt;
function c(v) {
  if (v===null || typeof v==='boolean' || typeof v==='string' || Number.isSafeInteger(v))
    return JSON.stringify(v);
  if (Array.isArray(v)) return '['+v.map(c).join(',')+']';
  return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+c(v[k])).join(',')+'}';
}
const payload=Buffer.from(c({
  schema:'cognitum.cog.release-withdrawal.v1',
  withdrawal,
}));
if (!crypto.verify(null,payload,key,Buffer.from(envelope.signature,'base64url')))
  process.exit(2);
process.stdout.write(payload);
""",
                str(signed),
                str(combined_registry),
            ],
            check=True,
            capture_output=True,
        )
        self.assertEqual(canonical_withdrawal_payload(record), node.stdout)

        tampered = copy.deepcopy(record)
        tampered["reasonCode"] = "different.reason"
        tampered_path = withdrawal_dir / "tampered-withdrawal.json"
        tampered_path.write_text(json.dumps(tampered))
        with self.assertRaises(ReleaseError):
            verify_withdrawal(
                argparse.Namespace(
                    release=self.release,
                    validity=self.validity,
                    withdrawal=tampered_path,
                    registry=combined_registry,
                )
            )

        release_only = withdrawal_dir / "release-only-trust.json"
        release_only.write_text(self.registry.read_text())
        with self.assertRaises(ReleaseError):
            verify_withdrawal(
                argparse.Namespace(
                    release=self.release,
                    validity=self.validity,
                    withdrawal=signed,
                    registry=release_only,
                )
            )

        withdrawal_as_release = copy.deepcopy(trust)
        misplaced_withdrawal = withdrawal_as_release["withdrawals"].pop()
        misplaced_withdrawal["purpose"] = "release"
        withdrawal_as_release["releases"].append(misplaced_withdrawal)
        withdrawal_as_release_path = (
            withdrawal_dir / "withdrawal-as-release.json"
        )
        withdrawal_as_release_path.write_text(json.dumps(withdrawal_as_release))
        with self.assertRaises(ReleaseError):
            verify_withdrawal(
                argparse.Namespace(
                    release=self.release,
                    validity=self.validity,
                    withdrawal=signed,
                    registry=withdrawal_as_release_path,
                )
            )

        decision = withdrawal_dir / "decision.json"
        withdrawal_projection_decision(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                withdrawal=signed,
                registry=combined_registry,
                expected_release_digest=record["releaseDigest"],
                checked_at="2026-07-29T20:00:00Z",
                output=decision,
            )
        )
        self.assertEqual(json.loads(decision.read_text())["state"], "DENIED")
        withdrawal_projection_decision(
            argparse.Namespace(
                release=self.release,
                validity=self.validity,
                withdrawal=tampered_path,
                registry=combined_registry,
                expected_release_digest=record["releaseDigest"],
                checked_at="2026-07-29T20:00:00Z",
                output=decision,
            )
        )
        quarantined = json.loads(decision.read_text())
        self.assertEqual(quarantined["state"], "QUARANTINED")
        self.assertEqual(quarantined["releaseDigest"], record["releaseDigest"])

    def test_protected_generation_bound_evidence_admission(self) -> None:
        bucket = "cognitum-20260110-cog-release-stg"
        content = "a" * 64
        output = self.output / "release-evidence-locations.json"
        args = argparse.Namespace(
            bucket_name=bucket,
            retention_period_seconds=2_592_000,
            retention_policy_locked="false",
            kind="release",
            uri=(
                f"gs://{bucket}/staging/cogs/releases/anomaly-detect/1.2.0/"
                f"aarch64/evidence/sha256/{content}/release-evidence.json"
            ),
            generation="1785355200123456",
            content_digest=f"sha256:{content}",
            sigstore_bundle_digest="sha256:" + "c" * 64,
            sigstore_transparency_log_verified="true",
            output=output,
        )
        admission(args)
        value = json.loads(output.read_text())
        self.assertEqual(value["schema"], EVIDENCE_LOCATIONS_SCHEMA)
        validate_evidence_locations(value)

        for name, mutation in (
            (
                "legacy-public-bucket",
                {
                    "bucket": {
                        **value["bucket"],
                        "name": "cognitum-apps",
                        "resource": (
                            "//storage.googleapis.com/projects/_/buckets/"
                            "cognitum-apps"
                        ),
                    }
                },
            ),
            (
                "mutable-generation",
                {"objects": [{**value["objects"][0], "generation": "0"}]},
            ),
            (
                "wrong-content-digest",
                {
                    "objects": [
                        {
                            **value["objects"][0],
                            "contentDigest": "sha256:" + "b" * 64,
                        }
                    ]
                },
            ),
            (
                "unprotected",
                {
                    "bucket": {
                        **value["bucket"],
                        "publicAccessPrevention": "inherited",
                    }
                },
            ),
        ):
            with self.subTest(name=name):
                changed = copy.deepcopy(value)
                changed.update(mutation)
                with self.assertRaises(ReleaseError):
                    validate_evidence_locations(changed)

    def test_release_binds_exact_runtime_integration_bytes_and_evidence(self) -> None:
        release = json.loads(self.release.read_text())
        policy = json.loads(POLICY.read_text())
        evidence = json.loads(self.evidence.read_text())
        runtime = release["runtimeIntegrations"]
        self.assertEqual(runtime, policy["runtimeIntegrations"])
        self.assertEqual(runtime, evidence["runtimeIntegrations"])
        self.assertEqual(
            runtime["manifest"], json.loads(self.integration_manifest.read_text())
        )
        self.assertEqual(runtime["staticWebsiteBundleDigest"], None)
        decision = json.loads((self.output / "policy-decision.json").read_text())
        integration_control = next(
            control for control in decision["controls"] if control["id"] == "I1"
        )
        self.assertEqual(integration_control["evidence"], runtime["manifestDigest"])
        self.assertEqual(
            canonical_payload(release),
            (self.output / "release-statement.json").read_bytes(),
        )

    def test_runtime_integration_sidecar_mutations_fail_closed(self) -> None:
        cases: list[tuple[str, Path | None, Path | None]] = []
        cases.append(("omitted", None, None))
        cases.append(("directory", self.directory / "integrations", None))

        renamed = self.directory / "renamed-integration-manifest.json"
        renamed.write_bytes(self.integration_manifest.read_bytes())
        cases.append(("renamed", renamed, None))

        symlink = self.directory / self.integration_manifest.name
        symlink.symlink_to(self.integration_manifest)
        cases.append(("symlink", symlink, None))

        oversized = self.directory / "oversized-integration-manifest.json"
        oversized.write_bytes(b" " * (MAX_MANIFEST_BYTES + 1))
        cases.append(("oversized", oversized, None))

        tampered_dir = self.directory / "tampered"
        tampered_dir.mkdir()
        tampered = tampered_dir / self.integration_manifest.name
        value = json.loads(self.integration_manifest.read_text())
        value["integrations"]["tailscale"] = {"enabled": True}
        tampered.write_bytes(canonical_bytes(value))
        cases.append(("tampered", tampered, None))

        whitespace_dir = self.directory / "whitespace"
        whitespace_dir.mkdir()
        whitespace = whitespace_dir / self.integration_manifest.name
        whitespace.write_bytes(self.integration_manifest.read_bytes() + b" ")
        cases.append(("alternate-bytes", whitespace, None))

        unexpected_bundle = self.directory / "unexpected.tar.gz"
        unexpected_bundle.write_bytes(b"not authorized")
        cases.append(
            ("unexpected-static-bundle", self.integration_manifest, unexpected_bundle)
        )

        for index, (name, manifest, website_bundle) in enumerate(cases):
            with self.subTest(name=name):
                args = argparse.Namespace(**vars(self.prepare_args))
                args.integration_manifest = manifest
                args.static_website_bundle = website_bundle
                args.output_dir = self.directory / f"mutation-{index}"
                with self.assertRaises(ReleaseError):
                    prepare(args)

    def test_oversized_but_otherwise_canonical_manifest_fails_closed(self) -> None:
        with (
            ROOT
            / "tests"
            / "fixtures"
            / "cog-integrations"
            / "valid"
            / "all-enabled.toml"
        ).open("rb") as stream:
            source = tomllib.load(stream)
        source["integrations"]["web_mcp"]["cors"]["allowed_origins"] = [
            f"https://origin-{index}-{'a' * 1500}.example" for index in range(32)
        ]
        manifest = normalize_manifest(source)
        payload = canonical_bytes(manifest)
        self.assertGreater(len(payload), MAX_MANIFEST_BYTES)
        oversized = self.directory / "oversized-canonical-manifest.json"
        oversized.write_bytes(payload)
        with self.assertRaises(ManifestValidationError) as raised:
            load_canonical_manifest(oversized)
        self.assertEqual(raised.exception.issues[0].code, "unsafe-sidecar")

    def test_static_website_bundle_is_required_and_digest_bound(self) -> None:
        with (
            ROOT
            / "tests"
            / "fixtures"
            / "cog-integrations"
            / "valid"
            / "all-enabled.toml"
        ).open("rb") as stream:
            source = tomllib.load(stream)
        source["cog"]["id"] = "anomaly-detect"
        source["cog"]["version"] = "1.2.0"
        manifest = normalize_manifest(source)
        manifest_bytes = canonical_bytes(manifest)
        manifest_hex = hashlib.sha256(manifest_bytes).hexdigest()
        manifest_path = self.directory / (
            "cog-anomaly-detect-integrations-v1-sha256-" f"{manifest_hex}.json"
        )
        manifest_path.write_bytes(manifest_bytes)

        bundle_bytes = b"deterministic public website fixture"
        bundle_hex = hashlib.sha256(bundle_bytes).hexdigest()
        bundle_path = self.directory / (
            f"cog-anomaly-detect-website-v1-sha256-{bundle_hex}.tar.gz"
        )
        bundle_path.write_bytes(bundle_bytes)

        policy = copy.deepcopy(json.loads(POLICY.read_text()))
        policy["runtimeIntegrations"] = {
            "manifest": manifest,
            "manifestDigest": f"sha256:{manifest_hex}",
            "staticWebsiteBundleDigest": f"sha256:{bundle_hex}",
        }
        policy_path = self.directory / "static-release-policy.json"
        policy_path.write_text(json.dumps(policy))
        validate_policy(policy)

        args = argparse.Namespace(**vars(self.prepare_args))
        args.policy = policy_path
        args.integration_manifest = manifest_path
        args.static_website_bundle = bundle_path
        args.output_dir = self.directory / "static-release"
        prepare(args)
        unsigned = json.loads((args.output_dir / "unsigned-release.json").read_text())
        validate_release(unsigned, signed=False)
        self.assertEqual(
            unsigned["runtimeIntegrations"]["staticWebsiteBundleDigest"],
            f"sha256:{bundle_hex}",
        )

        args.output_dir = self.directory / "missing-static"
        args.static_website_bundle = None
        with self.assertRaises(ReleaseError):
            prepare(args)

        tampered_dir = self.directory / "tampered-static"
        tampered_dir.mkdir()
        tampered_bundle = tampered_dir / bundle_path.name
        tampered_bundle.write_bytes(bundle_bytes + b"tamper")
        args.output_dir = self.directory / "bad-static"
        args.static_website_bundle = tampered_bundle
        with self.assertRaises(ReleaseError):
            prepare(args)

    def test_canonical_payload_matches_website_node_algorithm(self) -> None:
        release = json.loads(self.release.read_text())
        node_script = """
const fs=require('fs');
const release=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));
delete release.seededAt;
delete release.provenance.detachedSignature;
function c(v) {
  if (v===null || typeof v==='boolean' || typeof v==='string' || Number.isSafeInteger(v))
    return JSON.stringify(v);
  if (Array.isArray(v)) return '['+v.map(c).join(',')+']';
  if (!v || typeof v!=='object') throw new Error('unsafe canonical value');
  return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+c(v[k])).join(',')+'}';
}
process.stdout.write(c({schema:'cognitum.cog.release-provenance.v1',release}));
"""

        def node_payload(value: dict, name: str) -> bytes:
            path = self.directory / name
            path.write_text(json.dumps(value), encoding="utf-8")
            return subprocess.run(
                ["node", "-e", node_script, str(path)],
                check=True,
                capture_output=True,
            ).stdout

        self.assertEqual(
            canonical_payload(release),
            node_payload(release, "node-release.json"),
        )

        for index, safe_integer in enumerate((-(2**53 - 1), 2**53 - 1)):
            mutated = copy.deepcopy(release)
            mutated["runtimeIntegrations"]["manifest"][
                "safeIntegerProbe"
            ] = safe_integer
            self.assertEqual(
                canonical_payload(mutated),
                node_payload(mutated, f"node-safe-integer-{index}.json"),
            )

        for unsafe_number in (1.5, -(2**53), 2**53):
            mutated = copy.deepcopy(release)
            mutated["runtimeIntegrations"]["manifest"]["unsafeNumber"] = unsafe_number
            with self.assertRaises(ReleaseError):
                canonical_payload(mutated)

    def test_committed_public_fixture_verifies_in_python_and_node(self) -> None:
        release_path = PUBLIC_FIXTURE / "signed-release.json"
        trust_path = PUBLIC_FIXTURE / "release-trust-registry.json"
        bootstrap_path = PUBLIC_FIXTURE / "trust-bootstrap.json"
        trust_value = json.loads(trust_path.read_text())
        bootstrap_value = json.loads(bootstrap_path.read_text())
        verify_trust_registry(
            argparse.Namespace(
                bootstrap=bootstrap_path,
                registry=trust_path,
                expected_bootstrap_digest=bootstrap_digest(bootstrap_value),
                expected_registry_digest=registry_payload_digest(trust_value),
                minimum_sequence=1,
                previous_registry=None,
                checked_at="2026-07-29T12:01:00Z",
            )
        )
        verify(argparse.Namespace(release=release_path, registry=trust_path))
        release = json.loads(release_path.read_text())
        node = subprocess.run(
            [
                "node",
                "-e",
                """
const crypto=require('crypto');
const fs=require('fs');
const release=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));
const trust=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
const envelope=release.provenance.detachedSignature;
const key=trust.releases.find(k=>k.keyId===envelope.keyId).publicKeyPem;
delete release.provenance.detachedSignature;
delete release.seededAt;
function c(v) {
  if (v===null || typeof v==='boolean' || typeof v==='string' || Number.isSafeInteger(v))
    return JSON.stringify(v);
  if (Array.isArray(v)) return '['+v.map(c).join(',')+']';
  return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+c(v[k])).join(',')+'}';
}
const payload=Buffer.from(c({schema:'cognitum.cog.release-provenance.v1',release}));
const signature=Buffer.from(envelope.signature,'base64url');
if (!crypto.verify(null,payload,key,signature)) process.exit(2);
process.stdout.write(payload);
""",
                str(release_path),
                str(trust_path),
            ],
            check=True,
            capture_output=True,
        )
        self.assertEqual(canonical_payload(release), node.stdout)
        encoded = release_path.read_text() + trust_path.read_text()
        self.assertNotIn("PRIVATE KEY", encoded)
        self.assertEqual(
            hashlib.sha256(release_path.read_bytes()).hexdigest(),
            "e7e7c16b0aa5a6a78ce0f3a646218cfa45567893f6cbd1cf66c8f2a9a2a14382",
        )
        self.assertEqual(
            hashlib.sha256(trust_path.read_bytes()).hexdigest(),
            "b47856f9a940466d2961d04c65b202aa27f8060ac4b186284a53909f2070f8c8",
        )

    def test_tampering_or_workflow_substitution_fails_closed(self) -> None:
        tampered = json.loads(self.release.read_text())
        tampered["networkPolicy"]["egressAllowlist"] = ["example.com"]
        tampered_path = self.output / "tampered.json"
        tampered_path.write_text(json.dumps(tampered))
        with self.assertRaises(ReleaseError):
            verify(argparse.Namespace(release=tampered_path, registry=self.registry))

        integration_tamper = json.loads(self.release.read_text())
        runtime = integration_tamper["runtimeIntegrations"]
        runtime["manifest"]["integrations"]["website"] = {
            "enabled": True,
            "artifact": {
                "kind": "oci-image",
                "image": ("registry.example/cog@sha256:" + "2" * 64),
            },
            "port": 8080,
            "basePath": "/",
            "healthCheck": {
                "path": "/healthz",
                "intervalSeconds": 30,
                "timeoutSeconds": 5,
            },
            "auth": {
                "mode": "tenant-session",
                "audience": "cog-website",
            },
            "exposure": "private",
            "ingress": "internal",
        }
        runtime["manifestDigest"] = (
            "sha256:" + hashlib.sha256(canonical_bytes(runtime["manifest"])).hexdigest()
        )
        runtime["staticWebsiteBundleDigest"] = None
        integration_tamper["provenance"]["detachedSignature"]["payloadDigest"] = (
            payload_digest(integration_tamper)
        )
        integration_tamper_path = self.output / "integration-tamper.json"
        integration_tamper_path.write_text(json.dumps(integration_tamper))
        validate_release(integration_tamper, signed=True)
        with self.assertRaises(ReleaseError):
            verify(
                argparse.Namespace(
                    release=integration_tamper_path,
                    registry=self.registry,
                )
            )

        trust = json.loads(self.registry.read_text())
        trust["releases"][0]["buildWorkflows"] = [
            "cognitum-one/cogs/other.yml@main"
        ]
        untrusted = self.output / "untrusted.json"
        untrusted.write_text(json.dumps(trust))
        with self.assertRaises(ReleaseError):
            verify(argparse.Namespace(release=self.release, registry=untrusted))

    def test_incomplete_policy_and_failed_isolation_are_rejected(self) -> None:
        policy = json.loads(POLICY.read_text())
        policy["unexpectedAuthority"] = "deploy"
        with self.assertRaises(ReleaseError):
            validate_policy(policy)

        bad_digest = copy.deepcopy(json.loads(POLICY.read_text()))
        bad_digest["runtimeIntegrations"]["manifestDigest"] = "sha256:" + "0" * 64
        with self.assertRaises(ReleaseError):
            validate_policy(bad_digest)

        unexpected_bundle = copy.deepcopy(json.loads(POLICY.read_text()))
        unexpected_bundle["runtimeIntegrations"]["staticWebsiteBundleDigest"] = (
            "sha256:" + "1" * 64
        )
        with self.assertRaises(ReleaseError):
            validate_policy(unexpected_bundle)

        noncanonical = copy.deepcopy(json.loads(POLICY.read_text()))
        noncanonical["runtimeIntegrations"]["manifest"]["integrations"][
            "unexpected"
        ] = {"enabled": False}
        with self.assertRaises(ReleaseError):
            validate_policy(noncanonical)

        failed = json.loads(self.isolation.read_text())
        failed["runs"][-1]["refused"] = False
        self.isolation.write_text(json.dumps(failed))
        args = argparse.Namespace(**vars(self.prepare_args))
        args.output_dir = self.directory / "failed"
        with self.assertRaises(ReleaseError):
            prepare(args)

    def test_signature_and_key_type_are_strict(self) -> None:
        short_signature = self.directory / "short.b64"
        short_signature.write_text(base64.b64encode(b"short").decode())
        with self.assertRaises(ReleaseError):
            finalize(
                argparse.Namespace(
                    unsigned_release=self.output / "unsigned-release.json",
                    build_evidence=self.output / "build-evidence.json",
                    signature_file=short_signature,
                    output=self.output / "bad-evidence.json",
                    signed_release_output=self.output / "bad-release.json",
                )
            )

        rsa_private = self.directory / "rsa-private.pem"
        rsa_public = self.directory / "rsa-public.pem"
        run(
            "openssl",
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
            str(rsa_private),
        )
        run(
            "openssl",
            "pkey",
            "-in",
            str(rsa_private),
            "-pubout",
            "-out",
            str(rsa_public),
        )
        with self.assertRaises(ReleaseError):
            args = argparse.Namespace(**vars(self.registry_args))
            args.public_key = rsa_public
            args.output = self.output / "rsa-registry.json"
            registry(args)

        build_evidence = json.loads((self.output / "build-evidence.json").read_text())
        build_evidence["dependencyLockDigest"] = "sha256:" + "0" * 64
        mismatched = self.output / "mismatched-build-evidence.json"
        mismatched.write_text(json.dumps(build_evidence))
        with self.assertRaises(ReleaseError):
            finalize(
                argparse.Namespace(
                    unsigned_release=self.output / "unsigned-release.json",
                    build_evidence=mismatched,
                    signature_file=self.signature_b64,
                    output=self.output / "mismatched-evidence.json",
                    signed_release_output=self.output / "mismatched-release.json",
                )
            )

    def test_schemas_and_stage_only_workflow_contract(self) -> None:
        schemas = sorted(ROOT.glob("schemas/*.schema.json"))
        self.assertEqual(len(schemas), 13)
        encoded = json.dumps([json.loads(path.read_text()) for path in schemas])
        self.assertIn("cognitum.cog.release-provenance.v1", encoded)
        self.assertIn("Complete signed Cog release record v1", encoded)
        self.assertIn("runtimeIntegrations", encoded)
        self.assertIn("cognitum.cog.trust-registry.v3", encoded)
        self.assertIn("cognitum.cog.release-withdrawal.v1", encoded)
        self.assertIn("cognitum.cog.release-evidence-locations.v1", encoded)
        self.assertIn("ed25519", encoded)
        staging = (ROOT / ".github/workflows/publish-cog-staging.yml").read_text()
        production = (ROOT / ".github/workflows/publish-cog.yml").read_text()
        self.assertIn("EC_SIGN_ED25519", staging)
        self.assertIn("kms asymmetric-sign", staging)
        self.assertNotIn("--digest-algorithm", staging)
        self.assertIn("candidate-trust-registry.json", staging)
        self.assertIn("cosign", staging)
        self.assertNotIn("GCP_COGS_STAGING_SIGNING_KEY", production)
        self.assertNotIn("EC_SIGN_ED25519", production)
        self.assertNotIn("credentials_json", staging)
        checked = subprocess.run(
            [sys.executable, "scripts/cog_release_schema_check.py"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(checked.returncode, 0, checked.stderr)
        self.assertIn("validated 13 local schemas", checked.stdout)


class CogReleaseSchemaGateTests(unittest.TestCase):
    def test_gate_fails_closed_on_relative_or_unresolved_cross_file_refs(
        self,
    ) -> None:
        for name, reference in (
            (
                "relative",
                "./cognitum.cog.release-policy.v1.schema.json"
                "#/$defs/runtimeIntegrations",
            ),
            (
                "unresolved",
                "https://schemas.cognitum.one/missing-release-schema.json",
            ),
        ):
            with self.subTest(name=name):
                registry, by_path = schema_gate._load_schemas()
                by_path = copy.deepcopy(by_path)
                record_path = next(
                    path
                    for path in by_path
                    if path.name == "cognitum.cog.release-record.v1.schema.json"
                )
                by_path[record_path]["properties"]["runtimeIntegrations"][
                    "$ref"
                ] = reference
                registry = {schema["$id"]: schema for schema in by_path.values()}
                with (
                    patch.object(
                        schema_gate,
                        "_load_schemas",
                        return_value=(registry, by_path),
                    ),
                    self.assertRaises(schema_gate.SchemaCheckError),
                ):
                    schema_gate.check()

    def test_gate_fails_closed_on_invalid_public_fixture(self) -> None:
        read_json = schema_gate.read_json

        def invalid_fixture(path: Path) -> dict:
            value = read_json(path)
            if path.name == "signed-release.json":
                value.pop("runtimeIntegrations")
            return value

        with (
            patch.object(schema_gate, "read_json", side_effect=invalid_fixture),
            self.assertRaises(schema_gate.SchemaCheckError),
        ):
            schema_gate.check()


class CogReleaseWorkflowPolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.workflows = {
            name: (WORKFLOW_DIR / name).read_text()
            for name in (
                "ci.yml",
                "security.yml",
                "publish-cog.yml",
                "publish-cog-staging.yml",
                "withdraw-cog-staging.yml",
                "admit-cog-trust-staging.yml",
                "build-all-cogs.yml",
            )
        }

    @staticmethod
    def policy_problems(workflows: dict[str, str]) -> list[str]:
        problems: list[str] = []
        combined = "\n".join(workflows.values())
        for name, source in workflows.items():
            lines = source.splitlines()
            for index, line in enumerate(lines):
                stripped = line.strip()
                if "uses:" not in stripped:
                    continue
                reference = stripped.split("uses:", 1)[1].strip()
                revision = reference.rsplit("@", 1)[-1]
                if not __import__("re").fullmatch(r"[a-f0-9]{40}", revision):
                    problems.append(f"{name}: unpinned action {reference}")
                if not reference.startswith("actions/checkout@"):
                    continue
                indentation = len(line) - len(line.lstrip())
                step_lines: list[str] = []
                for candidate in lines[index + 1 :]:
                    candidate_indentation = len(candidate) - len(candidate.lstrip())
                    if (
                        candidate.strip().startswith("- ")
                        and candidate_indentation == indentation
                    ):
                        break
                    step_lines.append(candidate.strip())
                if "persist-credentials: false" not in step_lines:
                    problems.append(
                        f"{name}: checkout persists the workflow credential"
                    )

        ci = workflows["ci.yml"]
        if "\npermissions:\n  contents: read\n\njobs:" not in ci:
            problems.append("ci: workflow permissions are not read-only")
        staging = workflows["publish-cog-staging.yml"]
        withdrawal = workflows["withdraw-cog-staging.yml"]
        trust_admission = workflows["admit-cog-trust-staging.yml"]
        production = workflows["publish-cog.yml"]
        batch = workflows["build-all-cogs.yml"]
        sidecar_workflows = {
            "staging": staging,
            "production": production,
            "batch": batch,
        }
        for label, source in sidecar_workflows.items():
            if source.count("actual != expected") < 2:
                problems.append(f"{label}: exact sidecar-set rejection is incomplete")
            if (
                source.count(
                    "path.is_symlink() or not path.is_file() for path in expected"
                )
                < 2
            ):
                problems.append(f"{label}: sidecar symlink rejection is incomplete")
        for label, source in (("staging", staging), ("production", production)):
            if "cog_version" in source:
                problems.append(
                    f"{label}: unsafe manual release-version override remains"
                )
            if 'if [ "$CARGO_VERSION" != "$MANIFEST_VERSION" ]; then' not in source:
                problems.append(
                    f"{label}: Cargo.toml/cog.toml version equality is not mandatory"
                )
        for token in (
            "id: integrations",
            "id: website_bundle",
            '--integration-manifest "${{ steps.integrations.outputs.manifest }}"',
            'PREPARE+=(--static-website-bundle "${{ steps.website_bundle.outputs.bundle }}")',
            'upload_immutable "${{ steps.integrations.outputs.manifest }}"',
            'upload_immutable "${{ steps.integrations.outputs.checksum }}"',
            "cognitum.cog.trust-registry.v3",
            "--kms-key-version",
            "--protection-level SOFTWARE",
            "--purpose release",
            '--github-owner-id "${{ github.repository_owner_id }}"',
            '--github-repository-id "${{ github.repository_id }}"',
            "--github-workflow-id 322710413",
            "cognitum-20260110-cog-release-stg",
            "--if-generation-match=0",
            "--print-created-message",
            "candidate-release-evidence-locations.json",
            "--retention-policy-locked",
            "timeout --signal=TERM --kill-after=5s 120s",
            '"status": "UNKNOWN"',
            '"retryAllowed": False',
            "return 75",
            "do not retry or upsert; use the read-only auditor",
            "reconcile-cog-seed-staging.yml",
        ):
            if token not in staging:
                problems.append(f"staging: missing exact sidecar binding {token}")
        for token in (
            "environment: cogs-withdrawal-staging",
            "cogs-withdrawal-publisher-stg",
            "cog-withdrawal-publisher-stg@cognitum-20260110",
            "cog-withdrawal-stg/cryptoKeys/withdrawal-ed25519",
            "cognitum-20260110-cog-withdrawal-stg",
            'read_exact "$REGISTRY_URI" out/withdrawal/release-trust-registry.json \\\n'
            "            cognitum-20260110-cog-trust-stg",
            "verify-admitted-release",
            "key.get(\"purpose\") != \"withdrawal\"",
            "policy_decision_uri",
            "sigstore_bundle_uri",
            "require_transparency_log_bundle",
            'release["securityAttestation"][',
            '"policyDecisionDigest"',
            "--effective-at",
            "--if-generation-match=0",
            "timeout --signal=TERM --kill-after=5s 120s",
            "create result is UNKNOWN; never retry or upsert",
            '"status": "UNKNOWN"',
            '"retryAllowed": False',
            "reconcile-cog-seed-staging.yml",
            "exit 75",
            "No Firestore/runtime seed was attempted.",
        ):
            if token not in withdrawal:
                problems.append(f"withdrawal: missing authority separation {token}")
        if (
            "--purpose withdrawal" in staging
            or "cog-withdrawal-stg" in staging
            or "cog-withdrawal-publisher-stg" in staging
        ):
            problems.append("staging: release publisher retains withdrawal authority")
        if (
            "cog-release-publisher-stg@" in withdrawal
            or "cog-release-stg/cryptoKeys/release-ed25519" in withdrawal
        ):
            problems.append("withdrawal: release authority is reused")
        if "createDocument" in withdrawal or "datastore.entities" in withdrawal:
            problems.append("withdrawal: publisher contains projection-seeder authority")
        for token in (
            "environment: cogs-trust-admission-staging",
            "scripts/cog_trust_admission.py",
            "GCP_COGS_TRUST_STAGING_APPROVED_WORKFLOW_SHA",
            "GCP_COGS_TRUST_STAGING_APPROVED_REGISTRY_DIGEST",
            "GCP_COGS_TRUST_STAGING_REGISTRY_HEAD_SEQUENCE",
            "GCP_COGS_TRUST_STAGING_REGISTRY_HEAD_DIGEST",
            "--if-generation-match=0",
            '"status": "UNKNOWN"',
            '"retryAllowed": False',
            "never retry, overwrite, or use an alternate path",
            '"receiptAttestation": "PENDING_EXTERNAL"',
            '"deploymentAuthority": False',
        ):
            if token not in trust_admission:
                problems.append(
                    f"trust-admission: missing fail-closed append control {token}"
                )
        for forbidden in (
            "kms asymmetric-sign",
            "gcloud storage ls",
            "gcloud storage cat",
            "gcloud storage rm",
            "gcloud storage mv",
            "createDocument",
            "datastore.entities",
            "workload-identity-pools providers update",
            "--if-generation-match=1",
            "credentials_json",
        ):
            if forbidden in trust_admission:
                problems.append(
                    f"trust-admission: forbidden authority remains: {forbidden}"
                )
        if trust_admission.count('"retryAllowed": False') != 2:
            problems.append(
                "trust-admission: every terminal outcome must remain non-retryable"
            )
        if "gs://cognitum-apps" in staging:
            problems.append("staging: legacy public evidence bucket remains")
        if "Refuse unsigned production publication (ADR-155 freeze)" not in production:
            problems.append("production: freeze gate missing")
        if (
            "google-github-actions/auth@" in production
            or "environment: cogs-production" in production
            or "GCP_COGS_PROD_" in production
        ):
            problems.append("production: frozen job still has cloud authority")
        if "Refuse unsigned batch publication (ADR-155 freeze)" not in batch:
            problems.append("batch: freeze gate missing")
        if (
            "google-github-actions/auth@" in batch
            or "environment: cogs-production" in batch
        ):
            problems.append("batch: frozen job still has cloud authority")
        for broad in ("find out/integrations", "find out/website"):
            if broad in combined:
                problems.append(f"workflow: broad sidecar discovery remains: {broad}")
        if "raw.githubusercontent.com/anchore/syft/main/install.sh" in production:
            problems.append("production: floating Syft main installer remains")
        if "cargo install cargo-audit --locked --quiet 2>/dev/null ||" in production:
            problems.append("production: unlocked cargo-audit fallback remains")
        for token in (
            "CARGO_AUDIT_VERSION: '0.22.2'",
            "SYFT_VERSION: '1.44.0'",
            "SYFT_SHA256: '0e91737aee2b5baf1d255b959630194a302335d848ff97bb07921eb6205b5f5a'",
        ):
            if token not in production:
                problems.append(f"production: missing pinned tool contract {token}")
        if (
            "security-scan.yml@62489f1606ce871af9c0405dd9e1cb6f886b15cc"
            not in workflows["security.yml"]
        ):
            problems.append("security: final merged org caller pin differs")
        return problems

    def test_workflow_release_policy_is_fail_closed(self) -> None:
        self.assertEqual(self.policy_problems(self.workflows), [])

    def test_workflow_policy_kills_security_mutations(self) -> None:
        mutations = {
            "unpinned-action": (
                "ci.yml",
                "actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
                "actions/checkout@v4",
            ),
            "persisted-checkout-credential": (
                "ci.yml",
                "persist-credentials: false",
                "persist-credentials: true",
            ),
            "write-capable-ci-token": (
                "ci.yml",
                "permissions:\n  contents: read\n\njobs:",
                "permissions:\n  contents: write\n\njobs:",
            ),
            "omitted-manifest": (
                "publish-cog-staging.yml",
                '--integration-manifest "${{ steps.integrations.outputs.manifest }}"',
                '--source-commit "${{ github.sha }}"',
            ),
            "broad-integrations": (
                "publish-cog-staging.yml",
                'upload_immutable "${{ steps.integrations.outputs.manifest }}"',
                "find out/integrations",
            ),
            "injected-sidecar": (
                "publish-cog-staging.yml",
                "actual != expected",
                "not expected.issubset(actual)",
            ),
            "symlink-sidecar": (
                "publish-cog-staging.yml",
                "path.is_symlink() or not path.is_file() for path in expected",
                "not path.is_file() for path in expected",
            ),
            "staging-version-override": (
                "publish-cog-staging.yml",
                'if [ "$CARGO_VERSION" != "$MANIFEST_VERSION" ]; then',
                "if false; then",
            ),
            "trust-v1-downgrade": (
                "publish-cog-staging.yml",
                "cognitum.cog.trust-registry.v3",
                "cognitum.cog.release-trust.v2",
            ),
            "legacy-public-bucket": (
                "publish-cog-staging.yml",
                "cognitum-20260110-cog-release-stg",
                "cognitum-apps",
            ),
            "release-gains-withdrawal-purpose": (
                "publish-cog-staging.yml",
                "--purpose release",
                "--purpose withdrawal",
            ),
            "withdrawal-gains-release-purpose": (
                "withdraw-cog-staging.yml",
                'key.get("purpose") != "withdrawal"',
                'key.get("purpose") != "release"',
            ),
            "withdrawal-registry-reuses-release-bucket": (
                "withdraw-cog-staging.yml",
                'read_exact "$REGISTRY_URI" out/withdrawal/release-trust-registry.json \\\n'
                "            cognitum-20260110-cog-trust-stg",
                'read_exact "$REGISTRY_URI" out/withdrawal/release-trust-registry.json \\\n'
                "            cognitum-20260110-cog-release-stg",
            ),
            "mutable-evidence-upload": (
                "publish-cog-staging.yml",
                "--if-generation-match=0",
                "--if-generation-match=1",
            ),
            "unknown-becomes-retryable": (
                "publish-cog-staging.yml",
                '"retryAllowed": False',
                '"retryAllowed": True',
            ),
            "withdrawal-blind-retry": (
                "withdraw-cog-staging.yml",
                "create result is UNKNOWN; never retry or upsert",
                "create result is transient; retrying",
            ),
            "trust-admission-mutable-append": (
                "admit-cog-trust-staging.yml",
                "--if-generation-match=0",
                "--if-generation-match=1",
            ),
            "trust-admission-blind-retry": (
                "admit-cog-trust-staging.yml",
                '"retryAllowed": False',
                '"retryAllowed": True',
            ),
            "trust-admission-unapproved-sha": (
                "admit-cog-trust-staging.yml",
                "GCP_COGS_TRUST_STAGING_APPROVED_WORKFLOW_SHA",
                "GITHUB_SHA",
            ),
            "production-unfrozen": (
                "publish-cog.yml",
                "Refuse unsigned production publication (ADR-155 freeze)",
                "Production publication",
            ),
            "batch-authority": (
                "build-all-cogs.yml",
                "permissions:\n      contents: read",
                "permissions:\n      contents: read\n      id-token: write\n    environment: cogs-production",
            ),
            "floating-syft": (
                "publish-cog.yml",
                "SYFT_VERSION: '1.44.0'",
                "raw.githubusercontent.com/anchore/syft/main/install.sh",
            ),
        }
        for name, (workflow, old, new) in mutations.items():
            with self.subTest(name=name):
                mutated = dict(self.workflows)
                self.assertIn(old, mutated[workflow])
                mutated[workflow] = mutated[workflow].replace(old, new, 1)
                self.assertTrue(self.policy_problems(mutated))


if __name__ == "__main__":
    unittest.main()
