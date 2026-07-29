#!/usr/bin/env python3
"""Build and verify website-compatible detached Cog release signatures."""

from __future__ import annotations

import argparse
import base64
import hashlib
import subprocess
import tempfile
from pathlib import Path

from cog_release_builder import prepare
from cog_release_provenance_lib import (
    DIGEST,
    EVIDENCE_LOCATIONS_SCHEMA,
    KMS_KEY_VERSION,
    KEY_ID,
    NUMERIC_GITHUB_ID,
    TRUST_SCHEMA,
    PROVENANCE_SCHEMA,
    WITHDRAWAL_SCHEMA,
    ReleaseError,
    canonical_payload,
    canonical_withdrawal_payload,
    exact_keys,
    payload_digest,
    read_json,
    require_ascii,
    require_match,
    require_object,
    require_strings,
    validate_evidence_locations,
    validate_release,
    validate_withdrawal,
    withdrawal_payload_digest,
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

WITHDRAWAL_BINDINGS = {
    "releaseDigest": ("releaseDigest",),
    "releasePayloadDigest": ("releasePayloadDigest",),
    "cogId": ("cogId",),
    "action": ("action",),
    "reasonCode": ("reasonCode",),
    "issuedAt": ("issuedAt",),
    "issuer": ("issuer",),
}


def _signature_bytes(path: Path) -> bytes:
    try:
        raw_text = path.read_text(encoding="ascii").strip()
        signature = base64.b64decode(raw_text, validate=True)
    except (ValueError, UnicodeError) as error:
        raise ReleaseError("KMS signature file is not canonical base64") from error
    if len(signature) != 64:
        raise ReleaseError("Ed25519 signature must be exactly 64 bytes")
    return signature


def _public_key_der(path: Path) -> bytes:
    inspected = subprocess.run(
        ["openssl", "pkey", "-pubin", "-in", str(path), "-outform", "DER"],
        check=False,
        capture_output=True,
    )
    if inspected.returncode or not inspected.stdout:
        raise ReleaseError("public key is not an Ed25519 key")
    detailed = subprocess.run(
        ["openssl", "pkey", "-pubin", "-in", str(path), "-text", "-noout"],
        check=False,
        capture_output=True,
        text=True,
    )
    if detailed.returncode or "ED25519" not in detailed.stdout.upper():
        raise ReleaseError("public key is not an Ed25519 key")
    return inspected.stdout


def _validated_trust_key(
    trust: dict,
    key_id: str,
    *,
    purpose: str,
) -> dict:
    exact_keys(trust, {"schema", "keys"}, "trust registry")
    if trust["schema"] != TRUST_SCHEMA:
        raise ReleaseError("unsupported trust registry schema")
    key = require_object(
        require_object(trust["keys"], "trust registry keys").get(key_id),
        "trusted key",
    )
    exact_keys(
        key,
        {
            "algorithm",
            "kmsAlgorithm",
            "kmsKeyVersion",
            "protectionLevel",
            "publicKeyFingerprint",
            "publicKeyPem",
            "status",
            "purposes",
            "builderIdentities",
            "buildWorkflows",
            "github",
        },
        "trusted key",
    )
    if (
        key["algorithm"] != "ed25519"
        or key["kmsAlgorithm"] != "EC_SIGN_ED25519"
        or key["protectionLevel"] != "software"
        or key["status"] != "active"
    ):
        raise ReleaseError("trusted key is not active SOFTWARE EC_SIGN_ED25519")
    require_match(key["kmsKeyVersion"], KMS_KEY_VERSION, "trusted KMS key version")
    if purpose not in require_strings(key["purposes"], "trusted key purposes"):
        raise ReleaseError(f"trusted key is not admitted for {purpose}")
    if set(key["purposes"]) - {"release", "withdrawal"}:
        raise ReleaseError("trusted key contains an unsupported purpose")

    pem = key["publicKeyPem"]
    if (
        not isinstance(pem, str)
        or not 1 <= len(pem) <= 2_048
        or any(ord(character) > 0x7F for character in pem)
        or "PRIVATE KEY" in pem
    ):
        raise ReleaseError("trusted public key contains private key material")
    with tempfile.TemporaryDirectory() as directory:
        public_path = Path(directory) / "public.pem"
        public_path.write_text(pem, encoding="ascii")
        fingerprint = (
            "sha256:"
            f"{hashlib.sha256(_public_key_der(public_path)).hexdigest()}"
        )
    if require_match(
        key["publicKeyFingerprint"],
        DIGEST,
        "trusted public-key fingerprint",
    ) != fingerprint:
        raise ReleaseError("trusted public-key fingerprint does not match")

    github = require_object(key["github"], "trusted GitHub admission")
    exact_keys(
        github,
        {"ownerId", "repositoryId", "workflowIds"},
        "trusted GitHub admission",
    )
    require_match(github["ownerId"], NUMERIC_GITHUB_ID, "trusted GitHub owner id")
    require_match(
        github["repositoryId"],
        NUMERIC_GITHUB_ID,
        "trusted GitHub repository id",
    )
    workflow_ids = require_strings(
        github["workflowIds"], "trusted GitHub workflow ids"
    )
    for workflow_id in workflow_ids:
        require_match(
            workflow_id, NUMERIC_GITHUB_ID, "trusted GitHub workflow id"
        )
    return key


def _verify_ed25519(payload: bytes, envelope: dict, public_key_pem: str) -> None:
    try:
        signature = base64.urlsafe_b64decode(envelope["signature"] + "==")
    except ValueError as error:
        raise ReleaseError("signature is invalid base64url") from error
    with tempfile.TemporaryDirectory() as directory:
        temporary = Path(directory)
        payload_path = temporary / "statement.json"
        signature_path = temporary / "signature.bin"
        public_path = temporary / "public.pem"
        payload_path.write_bytes(payload)
        signature_path.write_bytes(signature)
        public_path.write_text(public_key_pem, encoding="ascii")
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
        raise ReleaseError("Ed25519 signature verification failed")


def finalize(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.unsigned_release), signed=False)
    signature_bytes = _signature_bytes(args.signature_file)
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
    public_key_fingerprint = (
        "sha256:"
        f"{hashlib.sha256(_public_key_der(args.public_key)).hexdigest()}"
    )
    kms_key_version = require_match(
        args.kms_key_version, KMS_KEY_VERSION, "KMS key version"
    )
    if args.kms_algorithm != "EC_SIGN_ED25519":
        raise ReleaseError("KMS algorithm must be EC_SIGN_ED25519")
    if args.protection_level.lower() != "software":
        raise ReleaseError("KMS protection level must be SOFTWARE")
    purposes = require_strings(args.purpose, "key purposes")
    if set(purposes) - {"release", "withdrawal"}:
        raise ReleaseError("key purpose is unsupported")
    owner_id = require_match(
        args.github_owner_id, NUMERIC_GITHUB_ID, "GitHub owner id"
    )
    repository_id = require_match(
        args.github_repository_id, NUMERIC_GITHUB_ID, "GitHub repository id"
    )
    workflow_ids = [
        require_match(value, NUMERIC_GITHUB_ID, "GitHub workflow id")
        for value in require_strings(args.github_workflow_id, "GitHub workflow ids")
    ]
    value = {
        "schema": TRUST_SCHEMA,
        "keys": {
            key_id: {
                "algorithm": "ed25519",
                "kmsAlgorithm": "EC_SIGN_ED25519",
                "kmsKeyVersion": kms_key_version,
                "protectionLevel": "software",
                "publicKeyFingerprint": public_key_fingerprint,
                "publicKeyPem": pem,
                "status": "active",
                "purposes": purposes,
                "builderIdentities": [
                    require_ascii(args.builder_identity, "builder identity")
                ],
                "buildWorkflows": [
                    require_ascii(args.build_workflow, "build workflow")
                ],
                "github": {
                    "ownerId": owner_id,
                    "repositoryId": repository_id,
                    "workflowIds": workflow_ids,
                },
            }
        },
    }
    _validated_trust_key(value, key_id, purpose=purposes[0])
    write_json(args.output, value)


def verify(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    trust = read_json(args.registry)
    envelope = release["provenance"]["detachedSignature"]
    key = _validated_trust_key(trust, envelope["keyId"], purpose="release")
    provenance = release["provenance"]
    if (
        provenance["builderIdentity"]
        not in require_strings(key["builderIdentities"], "trusted builder identities")
        or provenance["buildWorkflow"]
        not in require_strings(key["buildWorkflows"], "trusted build workflows")
    ):
        raise ReleaseError(
            "release signer is not trusted for this builder and workflow"
        )
    _verify_ed25519(canonical_payload(release), envelope, key["publicKeyPem"])


def prepare_withdrawal(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    verify(argparse.Namespace(release=args.release, registry=args.registry))
    release_signature = require_object(
        require_object(release["provenance"], "release provenance")[
            "detachedSignature"
        ],
        "release detached signature",
    )
    withdrawal = {
        "schema": WITHDRAWAL_SCHEMA,
        "releaseDigest": release["releaseDigest"],
        "releasePayloadDigest": release_signature["payloadDigest"],
        "cogId": release["cogId"],
        "action": args.action,
        "reasonCode": args.reason_code,
        "issuedAt": args.issued_at,
        "issuer": {
            "identity": require_ascii(args.issuer_identity, "issuer identity"),
            "workflow": require_ascii(args.issuer_workflow, "issuer workflow"),
            "githubOwnerId": require_match(
                args.github_owner_id, NUMERIC_GITHUB_ID, "GitHub owner id"
            ),
            "githubRepositoryId": require_match(
                args.github_repository_id,
                NUMERIC_GITHUB_ID,
                "GitHub repository id",
            ),
            "githubWorkflowId": require_match(
                args.github_workflow_id,
                NUMERIC_GITHUB_ID,
                "GitHub workflow id",
            ),
        },
    }
    validate_withdrawal(withdrawal, release, signed=False)
    key_id = require_match(args.key_id, KEY_ID, "withdrawal signing key id")
    trust = read_json(args.registry)
    key = _validated_trust_key(trust, key_id, purpose="withdrawal")
    issuer = withdrawal["issuer"]
    if (
        issuer["identity"] not in key["builderIdentities"]
        or issuer["workflow"] not in key["buildWorkflows"]
        or issuer["githubOwnerId"] != key["github"]["ownerId"]
        or issuer["githubRepositoryId"] != key["github"]["repositoryId"]
        or issuer["githubWorkflowId"] not in key["github"]["workflowIds"]
    ):
        raise ReleaseError("withdrawal issuer is not admitted by the signing key")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_json(args.output_dir / "unsigned-withdrawal.json", withdrawal)
    (args.output_dir / "withdrawal-statement.json").write_bytes(
        canonical_withdrawal_payload(withdrawal)
    )
    write_json(
        args.output_dir / "withdrawal-statement-metadata.json",
        {
            "schema": WITHDRAWAL_SCHEMA,
            "payloadDigest": withdrawal_payload_digest(withdrawal),
            "keyId": key_id,
        },
    )


def finalize_withdrawal(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    withdrawal = validate_withdrawal(
        read_json(args.unsigned_withdrawal),
        release,
        signed=False,
    )
    key_id = require_match(args.key_id, KEY_ID, "withdrawal signing key id")
    signature = _signature_bytes(args.signature_file)
    withdrawal["detachedSignature"] = {
        "schema": WITHDRAWAL_SCHEMA,
        "algorithm": "ed25519",
        "keyId": key_id,
        "payloadDigest": withdrawal_payload_digest(withdrawal),
        "signature": base64.urlsafe_b64encode(signature).decode("ascii").rstrip("="),
    }
    validate_withdrawal(withdrawal, release, signed=True)
    evidence = {
        field: _nested(withdrawal, path)
        for field, path in WITHDRAWAL_BINDINGS.items()
    }
    evidence["withdrawalSignature"] = withdrawal["detachedSignature"]
    evidence["withdrawalRecord"] = withdrawal
    write_json(args.output, evidence)
    write_json(args.signed_withdrawal_output, withdrawal)
    verify_withdrawal(
        argparse.Namespace(
            release=args.release,
            withdrawal=args.signed_withdrawal_output,
            registry=args.registry,
        )
    )


def _nested(value: dict, path: tuple[str, ...]) -> object:
    current: object = value
    for segment in path:
        current = require_object(current, "nested evidence binding")[segment]
    return current


def verify_withdrawal(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    withdrawal = validate_withdrawal(
        read_json(args.withdrawal),
        release,
        signed=True,
    )
    envelope = require_object(
        withdrawal["detachedSignature"], "withdrawal detached signature"
    )
    key = _validated_trust_key(
        read_json(args.registry),
        envelope["keyId"],
        purpose="withdrawal",
    )
    issuer = require_object(withdrawal["issuer"], "withdrawal issuer")
    if (
        issuer["identity"] not in key["builderIdentities"]
        or issuer["workflow"] not in key["buildWorkflows"]
        or issuer["githubOwnerId"] != key["github"]["ownerId"]
        or issuer["githubRepositoryId"] != key["github"]["repositoryId"]
        or issuer["githubWorkflowId"] not in key["github"]["workflowIds"]
    ):
        raise ReleaseError("withdrawal issuer is not admitted by the signing key")
    _verify_ed25519(
        canonical_withdrawal_payload(withdrawal),
        envelope,
        key["publicKeyPem"],
    )


def admission(args: argparse.Namespace) -> None:
    bucket_name = require_ascii(args.bucket_name, "evidence bucket name", 63)
    value = {
        "schema": EVIDENCE_LOCATIONS_SCHEMA,
        "bucket": {
            "name": bucket_name,
            "resource": (
                "//storage.googleapis.com/projects/_/buckets/"
                f"{bucket_name}"
            ),
            "publicAccessPrevention": "enforced",
            "uniformBucketLevelAccess": True,
            "retentionPeriodSeconds": args.retention_period_seconds,
            "retentionPolicyLocked": args.retention_policy_locked == "true",
            "versioningEnabled": True,
        },
        "objects": [
            {
                "kind": args.kind,
                "uri": args.uri,
                "generation": args.generation,
                "contentDigest": args.content_digest,
                "ifGenerationMatch": 0,
            }
        ],
    }
    validate_evidence_locations(value)
    write_json(args.output, value)


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
    registry_parser.add_argument("--kms-key-version", required=True)
    registry_parser.add_argument("--kms-algorithm", required=True)
    registry_parser.add_argument("--protection-level", required=True)
    registry_parser.add_argument(
        "--purpose",
        required=True,
        action="append",
        choices=("release", "withdrawal"),
    )
    registry_parser.add_argument("--builder-identity", required=True)
    registry_parser.add_argument("--build-workflow", required=True)
    registry_parser.add_argument("--github-owner-id", required=True)
    registry_parser.add_argument("--github-repository-id", required=True)
    registry_parser.add_argument(
        "--github-workflow-id",
        required=True,
        action="append",
    )
    registry_parser.add_argument("--output", required=True, type=Path)
    registry_parser.set_defaults(handler=registry)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--release", required=True, type=Path)
    verify_parser.add_argument("--registry", required=True, type=Path)
    verify_parser.set_defaults(handler=verify)

    prepare_withdrawal_parser = commands.add_parser("prepare-withdrawal")
    prepare_withdrawal_parser.add_argument("--release", required=True, type=Path)
    prepare_withdrawal_parser.add_argument("--registry", required=True, type=Path)
    prepare_withdrawal_parser.add_argument(
        "--action",
        required=True,
        choices=("withdrawn", "revoked"),
    )
    prepare_withdrawal_parser.add_argument("--reason-code", required=True)
    prepare_withdrawal_parser.add_argument("--issued-at", required=True)
    prepare_withdrawal_parser.add_argument("--key-id", required=True)
    prepare_withdrawal_parser.add_argument("--issuer-identity", required=True)
    prepare_withdrawal_parser.add_argument("--issuer-workflow", required=True)
    prepare_withdrawal_parser.add_argument("--github-owner-id", required=True)
    prepare_withdrawal_parser.add_argument("--github-repository-id", required=True)
    prepare_withdrawal_parser.add_argument("--github-workflow-id", required=True)
    prepare_withdrawal_parser.add_argument(
        "--output-dir",
        required=True,
        type=Path,
    )
    prepare_withdrawal_parser.set_defaults(handler=prepare_withdrawal)

    finalize_withdrawal_parser = commands.add_parser("finalize-withdrawal")
    finalize_withdrawal_parser.add_argument(
        "--release", required=True, type=Path
    )
    finalize_withdrawal_parser.add_argument(
        "--registry", required=True, type=Path
    )
    finalize_withdrawal_parser.add_argument(
        "--unsigned-withdrawal", required=True, type=Path
    )
    finalize_withdrawal_parser.add_argument(
        "--signature-file", required=True, type=Path
    )
    finalize_withdrawal_parser.add_argument("--key-id", required=True)
    finalize_withdrawal_parser.add_argument(
        "--output", required=True, type=Path
    )
    finalize_withdrawal_parser.add_argument(
        "--signed-withdrawal-output", required=True, type=Path
    )
    finalize_withdrawal_parser.set_defaults(handler=finalize_withdrawal)

    verify_withdrawal_parser = commands.add_parser("verify-withdrawal")
    verify_withdrawal_parser.add_argument(
        "--release", required=True, type=Path
    )
    verify_withdrawal_parser.add_argument(
        "--withdrawal", required=True, type=Path
    )
    verify_withdrawal_parser.add_argument(
        "--registry", required=True, type=Path
    )
    verify_withdrawal_parser.set_defaults(handler=verify_withdrawal)

    admission_parser = commands.add_parser("admission")
    admission_parser.add_argument("--bucket-name", required=True)
    admission_parser.add_argument(
        "--retention-period-seconds",
        required=True,
        type=int,
    )
    admission_parser.add_argument(
        "--retention-policy-locked",
        required=True,
        choices=("true", "false"),
    )
    admission_parser.add_argument(
        "--kind", required=True, choices=("release", "withdrawal")
    )
    admission_parser.add_argument("--uri", required=True)
    admission_parser.add_argument("--generation", required=True)
    admission_parser.add_argument("--content-digest", required=True)
    admission_parser.add_argument("--output", required=True, type=Path)
    admission_parser.set_defaults(handler=admission)
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
