"""Security and cross-runtime tests for detached Cog release provenance."""

from __future__ import annotations

import argparse
import base64
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from cog_release_provenance import finalize, registry, verify  # noqa: E402
from cog_release_builder import prepare  # noqa: E402
from cog_release_provenance_lib import (  # noqa: E402
    ReleaseError,
    canonical_payload,
    validate_policy,
)

POLICY = ROOT / "src" / "cogs" / "anomaly-detect" / "release-policy.json"
BUILDER = "github-actions://cognitum-one/cogs"
WORKFLOW = (
    "cognitum-one/cogs/.github/workflows/publish-cog-staging.yml"
    "@refs/heads/codex/cog-optional-web-tailscale-mcp"
)
KEY_ID = "gcp-kms:cogs-staging-release-2026-01"


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
            '{"mediaType":"application/vnd.dev.sigstore.bundle+json;version=0.3"}'
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
        prepare(
            argparse.Namespace(
                policy=POLICY,
                artifact=self.artifact,
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
        )
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
        registry(
            argparse.Namespace(
                public_key=self.public_key,
                key_id=KEY_ID,
                builder_identity=BUILDER,
                build_workflow=WORKFLOW,
                output=self.registry,
            )
        )
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

    def tearDown(self) -> None:
        self.temporary.cleanup()

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

    def test_canonical_payload_matches_website_node_algorithm(self) -> None:
        release = json.loads(self.release.read_text())
        node = subprocess.run(
            [
                "node",
                "-e",
                """
const fs=require('fs');
const release=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));
delete release.seededAt;
delete release.provenance.detachedSignature;
function c(v) {
  if (v===null || typeof v==='boolean' || typeof v==='string' || typeof v==='number')
    return JSON.stringify(v);
  if (Array.isArray(v)) return '['+v.map(c).join(',')+']';
  return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+c(v[k])).join(',')+'}';
}
process.stdout.write(c({schema:'cognitum.cog.release-provenance.v1',release}));
""",
                str(self.release),
            ],
            check=True,
            capture_output=True,
        )
        self.assertEqual(canonical_payload(release), node.stdout)

    def test_tampering_or_workflow_substitution_fails_closed(self) -> None:
        tampered = json.loads(self.release.read_text())
        tampered["networkPolicy"]["egressAllowlist"] = ["example.com"]
        tampered_path = self.output / "tampered.json"
        tampered_path.write_text(json.dumps(tampered))
        with self.assertRaises(ReleaseError):
            verify(argparse.Namespace(release=tampered_path, registry=self.registry))

        trust = json.loads(self.registry.read_text())
        trust["keys"][KEY_ID]["buildWorkflows"] = ["cognitum-one/cogs/other.yml@main"]
        untrusted = self.output / "untrusted.json"
        untrusted.write_text(json.dumps(trust))
        with self.assertRaises(ReleaseError):
            verify(argparse.Namespace(release=self.release, registry=untrusted))

    def test_incomplete_policy_and_failed_isolation_are_rejected(self) -> None:
        policy = json.loads(POLICY.read_text())
        policy["unexpectedAuthority"] = "deploy"
        with self.assertRaises(ReleaseError):
            validate_policy(policy)

        failed = json.loads(self.isolation.read_text())
        failed["runs"][-1]["refused"] = False
        self.isolation.write_text(json.dumps(failed))
        args = argparse.Namespace(
            policy=POLICY,
            artifact=self.artifact,
            sigstore_bundle=self.bundle,
            dependency_lock=self.lock,
            sbom=self.sbom,
            vulnerability_scan=self.vulnerability,
            provenance=self.provenance,
            isolation=self.isolation,
            output_dir=self.directory / "failed",
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
            registry(
                argparse.Namespace(
                    public_key=rsa_public,
                    key_id=KEY_ID,
                    builder_identity=BUILDER,
                    build_workflow=WORKFLOW,
                    output=self.output / "rsa-registry.json",
                )
            )

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
        schemas = sorted(ROOT.glob("schemas/cognitum.cog.release-*.schema.json"))
        self.assertEqual(len(schemas), 3)
        encoded = json.dumps([json.loads(path.read_text()) for path in schemas])
        self.assertIn("cognitum.cog.release-provenance.v1", encoded)
        self.assertIn("cognitum.cog.release-trust.v1", encoded)
        self.assertIn("ed25519", encoded)
        staging = (ROOT / ".github/workflows/publish-cog-staging.yml").read_text()
        production = (ROOT / ".github/workflows/publish-cog.yml").read_text()
        self.assertIn("EC_SIGN_ED25519", staging)
        self.assertIn("kms asymmetric-sign", staging)
        self.assertNotIn("--digest-algorithm", staging)
        self.assertIn("release-trust-registry.json", staging)
        self.assertIn("cosign", staging)
        self.assertNotIn("GCP_COGS_STAGING_SIGNING_KEY", production)
        self.assertNotIn("EC_SIGN_ED25519", production)
        self.assertNotIn("credentials_json", staging)


if __name__ == "__main__":
    unittest.main()
