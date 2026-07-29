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

from cog_release_provenance import finalize, registry, verify  # noqa: E402
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
    ReleaseError,
    canonical_payload,
    payload_digest,
    validate_release,
    validate_policy,
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
        verify(argparse.Namespace(release=release_path, registry=trust_path))
        release = json.loads(release_path.read_text())
        trust = json.loads(trust_path.read_text())
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
const key=trust.keys[envelope.keyId].publicKeyPem;
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
            "3ba71e669b41fc29579261469a166c1ddd92015fb54e68c0b43c375a550bbc1e",
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
        self.assertEqual(len(schemas), 4)
        encoded = json.dumps([json.loads(path.read_text()) for path in schemas])
        self.assertIn("cognitum.cog.release-provenance.v1", encoded)
        self.assertIn("Complete signed Cog release record v1", encoded)
        self.assertIn("runtimeIntegrations", encoded)
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
        checked = subprocess.run(
            [sys.executable, "scripts/cog_release_schema_check.py"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(checked.returncode, 0, checked.stderr)
        self.assertIn("validated 8 local schemas", checked.stdout)


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
        ):
            if token not in staging:
                problems.append(f"staging: missing exact sidecar binding {token}")
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
            "security-scan.yml@0288b22e56b262eb5a9bf190abf626f635ff887f"
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
