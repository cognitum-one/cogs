#!/usr/bin/env python3
"""Build and verify website-compatible detached Cog release signatures."""

from __future__ import annotations

import argparse
import base64
import subprocess
import tempfile
from pathlib import Path

from cog_release_builder import prepare
from cog_release_provenance_lib import (
    KEY_ID,
    TRUST_SCHEMA,
    PROVENANCE_SCHEMA,
    ReleaseError,
    canonical_payload,
    exact_keys,
    payload_digest,
    read_json,
    require_ascii,
    require_match,
    require_object,
    require_strings,
    validate_release,
    write_json,
)

EVIDENCE_BINDINGS = {
    "cogId": ("cogId",),
    "blueprintId": ("blueprintId",),
    "blueprintDigest": ("blueprintDigest",),
    "version": ("version",),
    "sourceCommit": ("sourceCommit",),
    "releaseDigest": ("releaseDigest",),
    "artifactDigest": ("releaseDigest",),
    "runtimeContractVersion": ("runtimeContractVersion",),
    "runtimeIntegrations": ("runtimeIntegrations",),
    "builderIdentity": ("provenance", "builderIdentity"),
    "buildWorkflow": ("provenance", "buildWorkflow"),
    "builtAt": ("provenance", "builtAt"),
    "dependencyLockDigest": ("provenance", "dependencyLockDigest"),
    "sbomDigest": ("provenance", "sbomDigest"),
    "provenanceDigest": ("provenance", "provenanceDigest"),
    "vulnerabilityScanDigest": ("securityAttestation", "vulnerabilityScanDigest"),
    "policyDecisionDigest": ("securityAttestation", "policyDecisionDigest"),
    "isolationEvidenceDigest": ("securityAttestation", "isolationEvidenceDigest"),
    "isolationPassed": ("securityAttestation", "isolationPassed"),
}


def finalize(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.unsigned_release), signed=False)
    raw_text = args.signature_file.read_text(encoding="ascii").strip()
    try:
        signature_bytes = base64.b64decode(raw_text, validate=True)
    except (ValueError, UnicodeError) as error:
        raise ReleaseError("KMS signature file is not canonical base64") from error
    if len(signature_bytes) != 64:
        raise ReleaseError("Ed25519 signature must be exactly 64 bytes")
    signature = base64.urlsafe_b64encode(signature_bytes).decode("ascii").rstrip("=")
    release["provenance"]["detachedSignature"] = {
        "schema": PROVENANCE_SCHEMA,
        "algorithm": "ed25519",
        "keyId": release["provenance"]["signingKeyId"],
        "payloadDigest": payload_digest(release),
        "signature": signature,
    }
    validate_release(release, signed=True)
    build_evidence = read_json(args.build_evidence)
    for field, path in EVIDENCE_BINDINGS.items():
        expected: object = release
        for segment in path:
            expected = expected[segment]  # type: ignore[index]
        if build_evidence.get(field) != expected:
            raise ReleaseError(f"build evidence {field} does not match signed release")
    envelope = release["provenance"]["detachedSignature"]
    build_evidence["releaseSignature"] = envelope
    build_evidence["releaseRecord"] = release
    write_json(args.output, build_evidence)
    write_json(args.signed_release_output, release)


def registry(args: argparse.Namespace) -> None:
    key_id = require_match(args.key_id, KEY_ID, "key id")
    pem = args.public_key.read_text(encoding="ascii")
    if len(pem) > 2_048 or "BEGIN PUBLIC KEY" not in pem or "PRIVATE KEY" in pem:
        raise ReleaseError(
            "public key file is malformed or contains private key material"
        )
    inspected = subprocess.run(
        ["openssl", "pkey", "-pubin", "-in", str(args.public_key), "-text", "-noout"],
        check=False,
        capture_output=True,
        text=True,
    )
    if inspected.returncode or "ED25519" not in inspected.stdout.upper():
        raise ReleaseError("public key is not an Ed25519 key")
    value = {
        "schema": TRUST_SCHEMA,
        "keys": {
            key_id: {
                "algorithm": "ed25519",
                "publicKeyPem": pem,
                "status": "active",
                "builderIdentities": [
                    require_ascii(args.builder_identity, "builder identity")
                ],
                "buildWorkflows": [
                    require_ascii(args.build_workflow, "build workflow")
                ],
            }
        },
    }
    write_json(args.output, value)


def verify(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    trust = read_json(args.registry)
    exact_keys(trust, {"schema", "keys"}, "trust registry")
    if trust["schema"] != TRUST_SCHEMA:
        raise ReleaseError("unsupported trust registry schema")
    envelope = release["provenance"]["detachedSignature"]
    key = require_object(
        require_object(trust["keys"], "trust registry keys").get(envelope["keyId"]),
        "trusted key",
    )
    exact_keys(
        key,
        {"algorithm", "publicKeyPem", "status", "builderIdentities", "buildWorkflows"},
        "trusted key",
    )
    provenance = release["provenance"]
    if (
        key["algorithm"] != "ed25519"
        or key["status"] != "active"
        or provenance["builderIdentity"]
        not in require_strings(key["builderIdentities"], "trusted builder identities")
        or provenance["buildWorkflow"]
        not in require_strings(key["buildWorkflows"], "trusted build workflows")
    ):
        raise ReleaseError(
            "release signer is not trusted for this builder and workflow"
        )
    try:
        signature = base64.urlsafe_b64decode(envelope["signature"] + "==")
    except ValueError as error:
        raise ReleaseError("release signature is invalid base64url") from error
    with tempfile.TemporaryDirectory() as directory:
        temporary = Path(directory)
        payload_path = temporary / "statement.json"
        signature_path = temporary / "signature.bin"
        public_path = temporary / "public.pem"
        payload_path.write_bytes(canonical_payload(release))
        signature_path.write_bytes(signature)
        public_path.write_text(key["publicKeyPem"], encoding="ascii")
        checked = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-inkey",
                str(public_path),
                "-rawin",
                "-in",
                str(payload_path),
                "-sigfile",
                str(signature_path),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
    if checked.returncode:
        raise ReleaseError("Ed25519 release signature verification failed")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    for name in (
        "policy",
        "artifact",
        "integration-manifest",
        "sigstore-bundle",
        "dependency-lock",
        "sbom",
        "vulnerability-scan",
        "provenance",
        "isolation",
        "output-dir",
    ):
        prepare_parser.add_argument(f"--{name}", required=True, type=Path)
    prepare_parser.add_argument("--static-website-bundle", type=Path)
    for name in (
        "arch",
        "version",
        "source-commit",
        "key-id",
        "builder-identity",
        "build-workflow",
        "built-at",
        "repository",
        "sigstore-tool-version",
    ):
        prepare_parser.add_argument(f"--{name}", required=True)
    prepare_parser.set_defaults(handler=prepare)
    final_parser = commands.add_parser("finalize")
    for name in (
        "unsigned-release",
        "build-evidence",
        "signature-file",
        "output",
        "signed-release-output",
    ):
        final_parser.add_argument(f"--{name}", required=True, type=Path)
    final_parser.set_defaults(handler=finalize)
    registry_parser = commands.add_parser("registry")
    registry_parser.add_argument("--public-key", required=True, type=Path)
    registry_parser.add_argument("--key-id", required=True)
    registry_parser.add_argument("--builder-identity", required=True)
    registry_parser.add_argument("--build-workflow", required=True)
    registry_parser.add_argument("--output", required=True, type=Path)
    registry_parser.set_defaults(handler=registry)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--release", required=True, type=Path)
    verify_parser.add_argument("--registry", required=True, type=Path)
    verify_parser.set_defaults(handler=verify)
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        args.handler(args)
    except (ReleaseError, OSError) as error:
        print(f"release provenance error: {error}", file=__import__("sys").stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
