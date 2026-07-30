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
    RELEASE_VALIDITY_SCHEMA,
    PROVENANCE_SCHEMA,
    WITHDRAWAL_SCHEMA,
    ReleaseError,
    canonical_payload,
    canonical_release_validity_payload,
    canonical_withdrawal_payload,
    exact_keys,
    payload_digest,
    parse_rfc3339,
    read_json,
    release_validity_payload_digest,
    require_ascii,
    require_digest,
    require_match,
    require_object,
    require_strings,
    validate_evidence_locations,
    validate_release,
    validate_release_validity,
    validate_withdrawal,
    withdrawal_is_effective,
    withdrawal_payload_digest,
    write_json,
)
from cog_trust_registry import (
    GENESIS,
    ROOT_KEY_ID,
    TRUST_REGISTRY_SCHEMA,
    bootstrap_digest,
    canonical_registry_payload,
    find_key,
    registry_payload_digest,
    validate_bootstrap,
    validate_registry,
)

TRUST_SCHEMA = TRUST_REGISTRY_SCHEMA

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
    "effectiveAt": ("effectiveAt",),
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
    key = find_key(trust, key_id, purpose=purpose)
    if (
        key["algorithm"] != "ed25519"
        or key["kmsAlgorithm"] != "EC_SIGN_ED25519"
        or key["protectionLevel"] != "software"
    ):
        raise ReleaseError("trusted key is not SOFTWARE EC_SIGN_ED25519")
    require_match(key["kmsKeyVersion"], KMS_KEY_VERSION, "trusted KMS key version")
    if key["purpose"] != purpose:
        raise ReleaseError(
            f"trusted key must be dedicated exclusively to {purpose}"
        )

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


def _require_key_accepts_signature_at(
    key: dict,
    signature_time: str,
    *,
    label: str,
) -> None:
    signed_at = parse_rfc3339(signature_time, f"{label} signature time")
    starts = parse_rfc3339(key["notBefore"], f"{label} key.notBefore")
    expires = parse_rfc3339(key["expiresAt"], f"{label} key.expiresAt")
    if not starts <= signed_at < expires:
        raise ReleaseError(f"{label} signature is outside the admitted key interval")
    if key["status"] == "active":
        return
    revocation = require_object(key["revocation"], f"{label} key revocation")
    scope = revocation["scope"]
    effective = parse_rfc3339(
        revocation["effectiveAt"], f"{label} key revocation.effectiveAt"
    )
    if scope == "all-signatures" or signed_at >= effective:
        raise ReleaseError(f"{label} signature is denied by key revocation")


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
    purposes = require_strings(args.purpose, "key purposes", maximum=1)
    if purposes not in (["release"], ["withdrawal"]):
        raise ReleaseError("key must have exactly one supported purpose")
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
    existing_registry = getattr(args, "existing_registry", None)
    if existing_registry is None:
        if args.sequence != 1 or args.previous_registry is not None:
            raise ReleaseError("genesis registry must use sequence 1 without predecessor")
        value = {
            "schema": TRUST_SCHEMA,
            "sequence": 1,
            "previousRegistryDigest": GENESIS,
            "issuedAt": args.issued_at,
            "notBefore": args.not_before,
            "expiresAt": args.expires_at,
            "releases": [],
            "withdrawals": [],
        }
    else:
        previous = validate_registry(read_json(existing_registry), signed=True)
        if args.previous_registry != existing_registry:
            raise ReleaseError("registry update must name its exact accepted predecessor")
        if args.sequence != previous["sequence"] + 1:
            raise ReleaseError("registry update sequence is not append-only")
        value = {
            "schema": TRUST_SCHEMA,
            "sequence": args.sequence,
            "previousRegistryDigest": registry_payload_digest(previous),
            "issuedAt": args.issued_at,
            "notBefore": args.not_before,
            "expiresAt": args.expires_at,
            "releases": list(previous["releases"]),
            "withdrawals": list(previous["withdrawals"]),
        }
    entries = value["releases"] + value["withdrawals"]
    if any(entry["keyId"] == key_id for entry in entries):
        raise ReleaseError("trust registry updates are append-only")
    entry = {
        "keyId": key_id,
        "algorithm": "ed25519",
        "kmsAlgorithm": "EC_SIGN_ED25519",
        "kmsKeyVersion": kms_key_version,
        "protectionLevel": "software",
        "publicKeyFingerprint": public_key_fingerprint,
        "publicKeyPem": pem,
        "status": "active",
        "purpose": purposes[0],
        "notBefore": args.key_not_before,
        "expiresAt": args.key_expires_at,
        "builderIdentities": [
            require_ascii(args.builder_identity, "builder identity")
        ],
        "buildWorkflows": [
            require_ascii(args.build_workflow, "build workflow")
        ],
        "workflowSha": require_match(
            args.workflow_sha,
            __import__("re").compile(r"^[a-f0-9]{40}$"),
            "workflow SHA",
        ),
        "github": {
            "ownerId": owner_id,
            "repositoryId": repository_id,
            "workflowIds": workflow_ids,
        },
        "revocation": None,
    }
    field = "releases" if purposes[0] == "release" else "withdrawals"
    value[field].append(entry)
    validate_registry(value, signed=False)
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
    if release["sourceCommit"] != key["workflowSha"]:
        raise ReleaseError("release source commit differs from admitted workflow SHA")
    signed_at = getattr(args, "signed_at", None)
    if signed_at is None:
        if key["status"] != "active":
            raise ReleaseError(
                "revoked release key requires a bound signature time"
            )
    else:
        _require_key_accepts_signature_at(
            key,
            signed_at,
            label="release",
        )
    _verify_ed25519(canonical_payload(release), envelope, key["publicKeyPem"])


def prepare_release_validity(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    verify(argparse.Namespace(release=args.release, registry=args.registry))
    envelope = require_object(
        require_object(release["provenance"], "release provenance")[
            "detachedSignature"
        ],
        "release detached signature",
    )
    validity = {
        "schema": RELEASE_VALIDITY_SCHEMA,
        "releaseDigest": release["releaseDigest"],
        "releasePayloadDigest": envelope["payloadDigest"],
        "cogId": release["cogId"],
        "issuedAt": args.issued_at,
        "signedAt": args.signed_at,
        "notBefore": args.not_before,
        "expiresAt": args.expires_at,
    }
    validate_release_validity(validity, release, signed=False)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_json(args.output_dir / "unsigned-release-validity.json", validity)
    (args.output_dir / "release-validity-statement.json").write_bytes(
        canonical_release_validity_payload(validity)
    )
    write_json(
        args.output_dir / "release-validity-statement-metadata.json",
        {
            "schema": RELEASE_VALIDITY_SCHEMA,
            "payloadDigest": release_validity_payload_digest(validity),
            "keyId": envelope["keyId"],
        },
    )


def finalize_release_validity(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    validity = validate_release_validity(
        read_json(args.unsigned_validity),
        release,
        signed=False,
    )
    envelope = require_object(
        require_object(release["provenance"], "release provenance")[
            "detachedSignature"
        ],
        "release detached signature",
    )
    signature = _signature_bytes(args.signature_file)
    validity["detachedSignature"] = {
        "schema": RELEASE_VALIDITY_SCHEMA,
        "algorithm": "ed25519",
        "keyId": envelope["keyId"],
        "payloadDigest": release_validity_payload_digest(validity),
        "signature": base64.urlsafe_b64encode(signature).decode("ascii").rstrip("="),
    }
    validate_release_validity(validity, release, signed=True)
    write_json(args.output, validity)
    verify_release_validity(
        argparse.Namespace(
            release=args.release,
            validity=args.output,
            registry=args.registry,
            checked_at=None,
        )
    )


def verify_release_validity(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    validity = validate_release_validity(
        read_json(args.validity),
        release,
        signed=True,
        checked_at=getattr(args, "checked_at", None),
    )
    envelope = require_object(
        validity["detachedSignature"], "release validity signature"
    )
    key = _validated_trust_key(
        read_json(args.registry),
        envelope["keyId"],
        purpose="release",
    )
    _require_key_accepts_signature_at(
        key,
        validity["signedAt"],
        label="release validity",
    )
    _verify_ed25519(
        canonical_release_validity_payload(validity),
        envelope,
        key["publicKeyPem"],
    )


def _validated_bootstrap_root(bootstrap: dict, key_id: str) -> dict:
    validate_bootstrap(bootstrap)
    matches = [root for root in bootstrap["roots"] if root["keyId"] == key_id]
    if len(matches) != 1:
        raise ReleaseError("trust registry signer is not in the source bootstrap")
    root = matches[0]
    pem = root["publicKeyPem"]
    with tempfile.TemporaryDirectory() as directory:
        public_path = Path(directory) / "root.pem"
        public_path.write_text(pem, encoding="ascii")
        fingerprint = (
            "sha256:"
            f"{hashlib.sha256(_public_key_der(public_path)).hexdigest()}"
        )
    if fingerprint != root["publicKeyFingerprint"]:
        raise ReleaseError("trust root public-key fingerprint does not match")
    return root


def finalize_trust_registry(args: argparse.Namespace) -> None:
    registry_value = validate_registry(
        read_json(args.unsigned_registry), signed=False
    )
    signatures: list[dict] = []
    for specification in args.signature:
        if "=" not in specification:
            raise ReleaseError("trust registry signature must be KEY_ID=PATH")
        key_id, raw_path = specification.split("=", 1)
        key_id = require_match(key_id, ROOT_KEY_ID, "trust registry signer id")
        signature = _signature_bytes(Path(raw_path))
        signatures.append(
            {
                "schema": TRUST_SCHEMA,
                "algorithm": "ed25519",
                "keyId": key_id,
                "payloadDigest": registry_payload_digest(registry_value),
                "signature": (
                    base64.urlsafe_b64encode(signature)
                    .decode("ascii")
                    .rstrip("=")
                ),
            }
        )
    registry_value["signatures"] = signatures
    validate_registry(registry_value, signed=True)
    write_json(args.output, registry_value)
    verify_trust_registry(
        argparse.Namespace(
            bootstrap=args.bootstrap,
            registry=args.output,
            expected_bootstrap_digest=args.expected_bootstrap_digest,
            expected_registry_digest=registry_payload_digest(registry_value),
            minimum_sequence=args.minimum_sequence,
            previous_registry=args.previous_registry,
            checked_at=args.checked_at,
        )
    )


def _verify_registry_signatures(bootstrap: dict, registry_value: dict) -> None:
    signer_roles: set[str] = set()
    for envelope in registry_value["signatures"]:
        root = _validated_bootstrap_root(bootstrap, envelope["keyId"])
        if root["role"] in signer_roles:
            raise ReleaseError("trust registry custodians are not distinct")
        _verify_ed25519(
            canonical_registry_payload(registry_value),
            envelope,
            root["publicKeyPem"],
        )
        signer_roles.add(root["role"])
    if len(signer_roles) < bootstrap["threshold"]:
        raise ReleaseError("trust registry does not satisfy root quorum")


def verify_trust_registry(args: argparse.Namespace) -> None:
    from cog_release_provenance_lib import validity_is_current

    bootstrap = validate_bootstrap(read_json(args.bootstrap))
    for root in bootstrap["roots"]:
        _validated_bootstrap_root(bootstrap, root["keyId"])
    registry_value = validate_registry(read_json(args.registry), signed=True)
    for field, purpose in (("releases", "release"), ("withdrawals", "withdrawal")):
        for key in registry_value[field]:
            _validated_trust_key(
                registry_value,
                key["keyId"],
                purpose=purpose,
            )
    actual_bootstrap_digest = bootstrap_digest(bootstrap)
    if require_digest(
        args.expected_bootstrap_digest, "expected trust bootstrap digest"
    ) != actual_bootstrap_digest:
        raise ReleaseError("trust bootstrap does not match the source pin")
    actual_registry_digest = registry_payload_digest(registry_value)
    if require_digest(
        args.expected_registry_digest, "expected trust registry digest"
    ) != actual_registry_digest:
        raise ReleaseError("trust registry does not match the runtime digest pin")
    minimum_sequence = args.minimum_sequence
    if (
        not isinstance(minimum_sequence, int)
        or isinstance(minimum_sequence, bool)
        or registry_value["sequence"] < minimum_sequence
    ):
        raise ReleaseError("trust registry is below the monotonic runtime pin")
    previous_path = getattr(args, "previous_registry", None)
    if registry_value["sequence"] > 1:
        if previous_path is None:
            raise ReleaseError("trust registry predecessor is required")
        previous = validate_registry(read_json(previous_path), signed=True)
        if (
            previous["sequence"] + 1 != registry_value["sequence"]
            or registry_payload_digest(previous)
            != registry_value["previousRegistryDigest"]
        ):
            raise ReleaseError("trust registry append-only chain is invalid")
        _verify_registry_signatures(bootstrap, previous)
    elif previous_path is not None:
        raise ReleaseError("genesis trust registry cannot have a predecessor")
    if not validity_is_current(
        issued_at=registry_value["issuedAt"],
        not_before=registry_value["notBefore"],
        expires_at=registry_value["expiresAt"],
        checked_at=args.checked_at,
        label="trust registry",
        maximum_lifetime_seconds=90 * 24 * 60 * 60,
    ):
        raise ReleaseError("trust registry is not current")
    _verify_registry_signatures(bootstrap, registry_value)


def verify_admitted_release(args: argparse.Namespace) -> None:
    verify_trust_registry(
        argparse.Namespace(
            bootstrap=args.bootstrap,
            registry=args.registry,
            expected_bootstrap_digest=args.expected_bootstrap_digest,
            expected_registry_digest=args.expected_registry_digest,
            minimum_sequence=args.minimum_sequence,
            previous_registry=args.previous_registry,
            checked_at=args.checked_at,
        )
    )
    verify_release_validity(
        argparse.Namespace(
            release=args.release,
            validity=args.validity,
            registry=args.registry,
            checked_at=args.checked_at,
        )
    )
    validity = read_json(args.validity)
    verify(
        argparse.Namespace(
            release=args.release,
            registry=args.registry,
            signed_at=validity["signedAt"],
        )
    )
    release = read_json(args.release)
    envelope = require_object(
        require_object(release["provenance"], "release provenance")[
            "detachedSignature"
        ],
        "release detached signature",
    )
    key = _validated_trust_key(
        read_json(args.registry), envelope["keyId"], purpose="release"
    )
    _require_key_accepts_signature_at(
        key,
        validity["signedAt"],
        label="release",
    )


def prepare_withdrawal(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    release_validity = validate_release_validity(
        read_json(args.validity), release, signed=True
    )
    verify_release_validity(
        argparse.Namespace(
            release=args.release,
            validity=args.validity,
            registry=args.registry,
            checked_at=None,
        )
    )
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
        "effectiveAt": args.effective_at,
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
            "workflowSha": require_match(
                args.workflow_sha,
                __import__("re").compile(r"^[a-f0-9]{40}$"),
                "GitHub workflow SHA",
            ),
        },
    }
    validate_withdrawal(
        withdrawal,
        release,
        signed=False,
        release_validity=release_validity,
    )
    key_id = require_match(args.key_id, KEY_ID, "withdrawal signing key id")
    trust = read_json(args.registry)
    key = _validated_trust_key(trust, key_id, purpose="withdrawal")
    release_key_id = release_signature["keyId"]
    release_key = _validated_trust_key(
        trust, release_key_id, purpose="release"
    )
    if (
        key_id == release_key_id
        or key["kmsKeyVersion"] == release_key["kmsKeyVersion"]
    ):
        raise ReleaseError(
            "withdrawal authority must use a distinct key and KMS key version"
        )
    _require_key_accepts_signature_at(
        key,
        withdrawal["issuedAt"],
        label="withdrawal",
    )
    issuer = withdrawal["issuer"]
    if (
        issuer["identity"] not in key["builderIdentities"]
        or issuer["workflow"] not in key["buildWorkflows"]
        or issuer["githubOwnerId"] != key["github"]["ownerId"]
        or issuer["githubRepositoryId"] != key["github"]["repositoryId"]
        or issuer["githubWorkflowId"] not in key["github"]["workflowIds"]
        or issuer["workflowSha"] != key["workflowSha"]
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
    release_validity = validate_release_validity(
        read_json(args.validity), release, signed=True
    )
    withdrawal = validate_withdrawal(
        read_json(args.unsigned_withdrawal),
        release,
        signed=False,
        release_validity=release_validity,
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
    validate_withdrawal(
        withdrawal,
        release,
        signed=True,
        release_validity=release_validity,
    )
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
            validity=args.validity,
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
    release_validity = validate_release_validity(
        read_json(args.validity), release, signed=True
    )
    withdrawal = validate_withdrawal(
        read_json(args.withdrawal),
        release,
        signed=True,
        release_validity=release_validity,
    )
    envelope = require_object(
        withdrawal["detachedSignature"], "withdrawal detached signature"
    )
    key = _validated_trust_key(
        read_json(args.registry),
        envelope["keyId"],
        purpose="withdrawal",
    )
    trust = read_json(args.registry)
    release_envelope = require_object(
        require_object(release["provenance"], "release provenance")[
            "detachedSignature"
        ],
        "release detached signature",
    )
    release_key = _validated_trust_key(
        trust,
        release_envelope["keyId"],
        purpose="release",
    )
    if (
        envelope["keyId"] == release_envelope["keyId"]
        or key["kmsKeyVersion"] == release_key["kmsKeyVersion"]
    ):
        raise ReleaseError(
            "withdrawal authority must be distinct from release authority"
        )
    _require_key_accepts_signature_at(
        key,
        withdrawal["issuedAt"],
        label="withdrawal",
    )
    issuer = require_object(withdrawal["issuer"], "withdrawal issuer")
    if (
        issuer["identity"] not in key["builderIdentities"]
        or issuer["workflow"] not in key["buildWorkflows"]
        or issuer["githubOwnerId"] != key["github"]["ownerId"]
        or issuer["githubRepositoryId"] != key["github"]["repositoryId"]
        or issuer["githubWorkflowId"] not in key["github"]["workflowIds"]
        or issuer["workflowSha"] != key["workflowSha"]
    ):
        raise ReleaseError("withdrawal issuer is not admitted by the signing key")
    _verify_ed25519(
        canonical_withdrawal_payload(withdrawal),
        envelope,
        key["publicKeyPem"],
    )


def withdrawal_projection_decision(args: argparse.Namespace) -> None:
    release = validate_release(read_json(args.release), signed=True)
    expected = require_digest(
        args.expected_release_digest, "withdrawal projection document id"
    )
    if expected != release["releaseDigest"]:
        raise ReleaseError("withdrawal projection document id differs from release")
    state = "QUARANTINED"
    reason = "verification-failed"
    try:
        verify_withdrawal(
            argparse.Namespace(
                release=args.release,
                validity=args.validity,
                withdrawal=args.withdrawal,
                registry=args.registry,
            )
        )
        withdrawal = read_json(args.withdrawal)
        if withdrawal_is_effective(withdrawal, args.checked_at):
            state = "DENIED"
            reason = "verified-effective-withdrawal"
        else:
            state = "PENDING_EFFECTIVE"
            reason = "verified-future-withdrawal"
    except (ReleaseError, OSError):
        # A malformed immutable projection cannot be interpreted as absence.
        # Quarantine only the document's bound release digest; never all Cogs.
        pass
    write_json(
        args.output,
        {
            "schema": "cognitum.cog.withdrawal-projection-decision.v1",
            "releaseDigest": expected,
            "state": state,
            "reason": reason,
            "checkedAt": args.checked_at,
        },
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
        "integrity": {
            "kmsSignatureVerified": True,
            "quorumRegistryRequired": True,
            "sigstoreTransparencyLogVerified": (
                args.sigstore_transparency_log_verified == "true"
            ),
            "sigstoreBundleDigest": args.sigstore_bundle_digest,
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
    registry_parser.add_argument("--workflow-sha", required=True)
    registry_parser.add_argument("--sequence", required=True, type=int)
    registry_parser.add_argument("--previous-registry", type=Path)
    registry_parser.add_argument("--issued-at", required=True)
    registry_parser.add_argument("--not-before", required=True)
    registry_parser.add_argument("--expires-at", required=True)
    registry_parser.add_argument("--key-not-before", required=True)
    registry_parser.add_argument("--key-expires-at", required=True)
    registry_parser.add_argument("--github-owner-id", required=True)
    registry_parser.add_argument("--github-repository-id", required=True)
    registry_parser.add_argument(
        "--github-workflow-id",
        required=True,
        action="append",
    )
    registry_parser.add_argument("--existing-registry", type=Path)
    registry_parser.add_argument("--output", required=True, type=Path)
    registry_parser.set_defaults(handler=registry)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--release", required=True, type=Path)
    verify_parser.add_argument("--registry", required=True, type=Path)
    verify_parser.set_defaults(handler=verify)

    prepare_validity_parser = commands.add_parser("prepare-release-validity")
    prepare_validity_parser.add_argument("--release", required=True, type=Path)
    prepare_validity_parser.add_argument("--registry", required=True, type=Path)
    prepare_validity_parser.add_argument("--issued-at", required=True)
    prepare_validity_parser.add_argument("--signed-at", required=True)
    prepare_validity_parser.add_argument("--not-before", required=True)
    prepare_validity_parser.add_argument("--expires-at", required=True)
    prepare_validity_parser.add_argument(
        "--output-dir", required=True, type=Path
    )
    prepare_validity_parser.set_defaults(handler=prepare_release_validity)

    finalize_validity_parser = commands.add_parser("finalize-release-validity")
    finalize_validity_parser.add_argument("--release", required=True, type=Path)
    finalize_validity_parser.add_argument("--registry", required=True, type=Path)
    finalize_validity_parser.add_argument(
        "--unsigned-validity", required=True, type=Path
    )
    finalize_validity_parser.add_argument(
        "--signature-file", required=True, type=Path
    )
    finalize_validity_parser.add_argument("--output", required=True, type=Path)
    finalize_validity_parser.set_defaults(handler=finalize_release_validity)

    verify_validity_parser = commands.add_parser("verify-release-validity")
    verify_validity_parser.add_argument("--release", required=True, type=Path)
    verify_validity_parser.add_argument("--validity", required=True, type=Path)
    verify_validity_parser.add_argument("--registry", required=True, type=Path)
    verify_validity_parser.add_argument("--checked-at")
    verify_validity_parser.set_defaults(handler=verify_release_validity)

    finalize_registry_parser = commands.add_parser("finalize-trust-registry")
    finalize_registry_parser.add_argument(
        "--bootstrap", required=True, type=Path
    )
    finalize_registry_parser.add_argument(
        "--unsigned-registry", required=True, type=Path
    )
    finalize_registry_parser.add_argument(
        "--signature", required=True, action="append"
    )
    finalize_registry_parser.add_argument(
        "--expected-bootstrap-digest", required=True
    )
    finalize_registry_parser.add_argument(
        "--minimum-sequence", required=True, type=int
    )
    finalize_registry_parser.add_argument("--previous-registry", type=Path)
    finalize_registry_parser.add_argument("--checked-at", required=True)
    finalize_registry_parser.add_argument("--output", required=True, type=Path)
    finalize_registry_parser.set_defaults(handler=finalize_trust_registry)

    verify_registry_parser = commands.add_parser("verify-trust-registry")
    verify_registry_parser.add_argument("--bootstrap", required=True, type=Path)
    verify_registry_parser.add_argument("--registry", required=True, type=Path)
    verify_registry_parser.add_argument(
        "--expected-bootstrap-digest", required=True
    )
    verify_registry_parser.add_argument(
        "--expected-registry-digest", required=True
    )
    verify_registry_parser.add_argument(
        "--minimum-sequence", required=True, type=int
    )
    verify_registry_parser.add_argument("--previous-registry", type=Path)
    verify_registry_parser.add_argument("--checked-at", required=True)
    verify_registry_parser.set_defaults(handler=verify_trust_registry)

    verify_admitted_parser = commands.add_parser("verify-admitted-release")
    verify_admitted_parser.add_argument("--release", required=True, type=Path)
    verify_admitted_parser.add_argument("--validity", required=True, type=Path)
    verify_admitted_parser.add_argument("--registry", required=True, type=Path)
    verify_admitted_parser.add_argument("--bootstrap", required=True, type=Path)
    verify_admitted_parser.add_argument(
        "--expected-bootstrap-digest", required=True
    )
    verify_admitted_parser.add_argument(
        "--minimum-sequence", required=True, type=int
    )
    verify_admitted_parser.add_argument("--previous-registry", type=Path)
    verify_admitted_parser.add_argument(
        "--expected-registry-digest", required=True
    )
    verify_admitted_parser.add_argument("--checked-at", required=True)
    verify_admitted_parser.set_defaults(handler=verify_admitted_release)

    prepare_withdrawal_parser = commands.add_parser("prepare-withdrawal")
    prepare_withdrawal_parser.add_argument("--release", required=True, type=Path)
    prepare_withdrawal_parser.add_argument("--validity", required=True, type=Path)
    prepare_withdrawal_parser.add_argument("--registry", required=True, type=Path)
    prepare_withdrawal_parser.add_argument(
        "--action",
        required=True,
        choices=("withdrawn", "revoked"),
    )
    prepare_withdrawal_parser.add_argument("--reason-code", required=True)
    prepare_withdrawal_parser.add_argument("--issued-at", required=True)
    prepare_withdrawal_parser.add_argument("--effective-at", required=True)
    prepare_withdrawal_parser.add_argument("--key-id", required=True)
    prepare_withdrawal_parser.add_argument("--issuer-identity", required=True)
    prepare_withdrawal_parser.add_argument("--issuer-workflow", required=True)
    prepare_withdrawal_parser.add_argument("--github-owner-id", required=True)
    prepare_withdrawal_parser.add_argument("--github-repository-id", required=True)
    prepare_withdrawal_parser.add_argument("--github-workflow-id", required=True)
    prepare_withdrawal_parser.add_argument("--workflow-sha", required=True)
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
        "--validity", required=True, type=Path
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
        "--validity", required=True, type=Path
    )
    verify_withdrawal_parser.add_argument(
        "--withdrawal", required=True, type=Path
    )
    verify_withdrawal_parser.add_argument(
        "--registry", required=True, type=Path
    )
    verify_withdrawal_parser.set_defaults(handler=verify_withdrawal)

    projection_parser = commands.add_parser("withdrawal-projection-decision")
    projection_parser.add_argument("--release", required=True, type=Path)
    projection_parser.add_argument("--validity", required=True, type=Path)
    projection_parser.add_argument("--withdrawal", required=True, type=Path)
    projection_parser.add_argument("--registry", required=True, type=Path)
    projection_parser.add_argument("--expected-release-digest", required=True)
    projection_parser.add_argument("--checked-at", required=True)
    projection_parser.add_argument("--output", required=True, type=Path)
    projection_parser.set_defaults(handler=withdrawal_projection_decision)

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
    admission_parser.add_argument("--sigstore-bundle-digest", required=True)
    admission_parser.add_argument(
        "--sigstore-transparency-log-verified",
        required=True,
        choices=("true", "false"),
    )
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
