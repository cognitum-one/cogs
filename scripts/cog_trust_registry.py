#!/usr/bin/env python3
"""Strict source contract for the quorum-rooted Cog trust registry v3."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import timedelta
from typing import Any

from cog_release_provenance_lib import (
    KEY_ID,
    KMS_KEY_VERSION,
    MAX_CANONICAL_BYTES,
    MAX_CLOCK_SKEW_SECONDS,
    NUMERIC_GITHUB_ID,
    REASON_CODE,
    ReleaseError,
    exact_keys,
    parse_rfc3339,
    require_ascii,
    require_digest,
    require_match,
    require_object,
    require_strings,
)

TRUST_BOOTSTRAP_SCHEMA = "cognitum.cog.trust-bootstrap.v1"
TRUST_REGISTRY_SCHEMA = "cognitum.cog.trust-registry.v3"
GENESIS = "GENESIS"
ROOT_IDENTITIES = {
    "security-custodian": "security-custodian/cog-trust-root-a",
    "platform-custodian": "platform-custodian/cog-trust-root-b",
    "independent-auditor": "independent-auditor/cog-trust-root-c",
}
ROOT_KEY_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}$")
MAX_REGISTRY_LIFETIME_SECONDS = 90 * 24 * 60 * 60
MAX_KEY_ADMISSION_LIFETIME_SECONDS = 365 * 24 * 60 * 60


def _require_public_pem(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not 1 <= len(value) <= 2_048
        or any(ord(character) > 0x7F for character in value)
        or "BEGIN PUBLIC KEY" not in value
        or "PRIVATE KEY" in value
    ):
        raise ReleaseError(f"{label} is not a bounded public-key PEM")
    return value


def _registry_canonical_check(
    value: Any,
    state: dict[str, int],
    *,
    field: str | None = None,
    depth: int = 0,
) -> None:
    state["values"] += 1
    if state["values"] > 2_048 or depth > 12:
        raise ReleaseError("trust registry exceeds canonicalization limits")
    if value is None or isinstance(value, bool):
        return
    if (
        isinstance(value, int)
        and not isinstance(value, bool)
        and -(2**53 - 1) <= value <= 2**53 - 1
    ):
        return
    if isinstance(value, str):
        if field == "publicKeyPem":
            _require_public_pem(value, "canonical trust public key")
        else:
            require_ascii(value, "canonical trust string", 8_192)
        return
    if isinstance(value, list):
        if len(value) > 256:
            raise ReleaseError("canonical trust array is too large")
        for item in value:
            _registry_canonical_check(item, state, depth=depth + 1)
        return
    if isinstance(value, dict):
        if len(value) > 256:
            raise ReleaseError("canonical trust object is too large")
        for key, item in value.items():
            require_ascii(key, "canonical trust field", 128)
            _registry_canonical_check(
                item,
                state,
                field=key,
                depth=depth + 1,
            )
        return
    raise ReleaseError("unsupported canonical trust value")


def canonical_registry_payload(registry: dict[str, Any]) -> bytes:
    unsigned = dict(registry)
    unsigned.pop("signatures", None)
    statement = {"schema": TRUST_REGISTRY_SCHEMA, "registry": unsigned}
    _registry_canonical_check(statement, {"values": 0})
    encoded = json.dumps(
        statement,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    if len(encoded) > MAX_CANONICAL_BYTES:
        raise ReleaseError("canonical trust registry is too large")
    return encoded


def registry_payload_digest(registry: dict[str, Any]) -> str:
    return (
        "sha256:"
        f"{hashlib.sha256(canonical_registry_payload(registry)).hexdigest()}"
    )


def bootstrap_digest(bootstrap: dict[str, Any]) -> str:
    validate_bootstrap(bootstrap)
    encoded = json.dumps(
        bootstrap,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def validate_bootstrap(value: dict[str, Any]) -> dict[str, Any]:
    exact_keys(value, {"schema", "threshold", "roots"}, "trust bootstrap")
    if value["schema"] != TRUST_BOOTSTRAP_SCHEMA or value["threshold"] != 2:
        raise ReleaseError("trust bootstrap must be the exact 2-of-3 contract")
    roots = value["roots"]
    if not isinstance(roots, list) or len(roots) != 3:
        raise ReleaseError("trust bootstrap must contain exactly three roots")
    seen_roles: set[str] = set()
    seen_ids: set[str] = set()
    seen_resources: set[str] = set()
    seen_fingerprints: set[str] = set()
    for index, raw in enumerate(roots):
        root = require_object(raw, f"trust bootstrap root {index}")
        exact_keys(
            root,
            {
                "role",
                "keyId",
                "signingResource",
                "algorithm",
                "publicKeyFingerprint",
                "publicKeyPem",
            },
            f"trust bootstrap root {index}",
        )
        role = require_ascii(root["role"], f"trust bootstrap root {index} role")
        key_id = require_match(
            root["keyId"], ROOT_KEY_ID, f"trust bootstrap root {index} key id"
        )
        if ROOT_IDENTITIES.get(role) != key_id:
            raise ReleaseError("trust bootstrap root role/key identity is not approved")
        resource = require_match(
            root["signingResource"],
            KMS_KEY_VERSION,
            f"trust bootstrap root {index} signing resource",
        )
        if root["algorithm"] != "ed25519":
            raise ReleaseError("trust bootstrap root algorithm must be Ed25519")
        fingerprint = require_digest(
            root["publicKeyFingerprint"],
            f"trust bootstrap root {index} fingerprint",
        )
        _require_public_pem(
            root["publicKeyPem"], f"trust bootstrap root {index} PEM"
        )
        if (
            role in seen_roles
            or key_id in seen_ids
            or resource in seen_resources
            or fingerprint in seen_fingerprints
        ):
            raise ReleaseError("trust bootstrap roots must be independently unique")
        seen_roles.add(role)
        seen_ids.add(key_id)
        seen_resources.add(resource)
        seen_fingerprints.add(fingerprint)
    if seen_roles != set(ROOT_IDENTITIES):
        raise ReleaseError("trust bootstrap root roles are incomplete")
    return value


def _validate_key_entry(
    value: dict[str, Any],
    *,
    expected_purpose: str,
    label: str,
) -> dict[str, Any]:
    exact_keys(
        value,
        {
            "keyId",
            "algorithm",
            "kmsAlgorithm",
            "kmsKeyVersion",
            "protectionLevel",
            "publicKeyFingerprint",
            "publicKeyPem",
            "status",
            "purpose",
            "notBefore",
            "expiresAt",
            "builderIdentities",
            "buildWorkflows",
            "workflowSha",
            "github",
            "revocation",
        },
        label,
    )
    require_match(value["keyId"], KEY_ID, f"{label} key id")
    if (
        value["algorithm"] != "ed25519"
        or value["kmsAlgorithm"] != "EC_SIGN_ED25519"
        or value["protectionLevel"] != "software"
        or value["purpose"] != expected_purpose
    ):
        raise ReleaseError(f"{label} algorithm, protection, or purpose is invalid")
    require_match(value["kmsKeyVersion"], KMS_KEY_VERSION, f"{label} KMS version")
    require_digest(value["publicKeyFingerprint"], f"{label} fingerprint")
    _require_public_pem(value["publicKeyPem"], f"{label} public key")
    starts = parse_rfc3339(value["notBefore"], f"{label}.notBefore")
    expires = parse_rfc3339(value["expiresAt"], f"{label}.expiresAt")
    if (
        expires <= starts
        or expires - starts
        > timedelta(seconds=MAX_KEY_ADMISSION_LIFETIME_SECONDS)
    ):
        raise ReleaseError(f"{label} key validity is invalid")
    require_strings(value["builderIdentities"], f"{label} builder identities")
    require_strings(value["buildWorkflows"], f"{label} build workflows")
    require_match(value["workflowSha"], re.compile(r"^[a-f0-9]{40}$"), f"{label} workflow SHA")
    github = require_object(value["github"], f"{label} GitHub identity")
    exact_keys(
        github,
        {"ownerId", "repositoryId", "workflowIds"},
        f"{label} GitHub identity",
    )
    require_match(github["ownerId"], NUMERIC_GITHUB_ID, f"{label} owner id")
    require_match(
        github["repositoryId"], NUMERIC_GITHUB_ID, f"{label} repository id"
    )
    workflow_ids = require_strings(
        github["workflowIds"], f"{label} workflow ids"
    )
    for workflow_id in workflow_ids:
        require_match(workflow_id, NUMERIC_GITHUB_ID, f"{label} workflow id")
    if value["status"] == "active":
        if value["revocation"] is not None:
            raise ReleaseError(f"{label} active key cannot have revocation")
    elif value["status"] == "revoked":
        revocation = require_object(value["revocation"], f"{label} revocation")
        exact_keys(
            revocation,
            {"effectiveAt", "reasonCode", "scope"},
            f"{label} revocation",
        )
        parse_rfc3339(revocation["effectiveAt"], f"{label} revocation effectiveAt")
        require_match(
            revocation["reasonCode"], REASON_CODE, f"{label} revocation reason"
        )
        if revocation["scope"] not in {"future-signatures", "all-signatures"}:
            raise ReleaseError(f"{label} revocation scope is unsupported")
    else:
        raise ReleaseError(f"{label} status is unsupported")
    return value


def validate_registry(
    value: dict[str, Any],
    *,
    signed: bool,
) -> dict[str, Any]:
    allowed = {
        "schema",
        "sequence",
        "previousRegistryDigest",
        "issuedAt",
        "notBefore",
        "expiresAt",
        "releases",
        "withdrawals",
    }
    if signed:
        allowed.add("signatures")
    exact_keys(value, allowed, "trust registry")
    if value["schema"] != TRUST_REGISTRY_SCHEMA:
        raise ReleaseError("unsupported trust registry schema")
    sequence = value["sequence"]
    if (
        not isinstance(sequence, int)
        or isinstance(sequence, bool)
        or not 1 <= sequence <= 2**53 - 1
    ):
        raise ReleaseError("trust registry sequence is invalid")
    previous = value["previousRegistryDigest"]
    if sequence == 1:
        if previous != GENESIS:
            raise ReleaseError("genesis trust registry marker is invalid")
    else:
        require_digest(previous, "previous trust registry digest")
    issued = parse_rfc3339(value["issuedAt"], "trust registry.issuedAt")
    starts = parse_rfc3339(value["notBefore"], "trust registry.notBefore")
    expires = parse_rfc3339(value["expiresAt"], "trust registry.expiresAt")
    skew = timedelta(seconds=MAX_CLOCK_SKEW_SECONDS)
    if starts < issued - skew or starts > issued + skew:
        raise ReleaseError("trust registry notBefore exceeds issuance skew")
    if (
        expires <= starts
        or expires - issued > timedelta(seconds=MAX_REGISTRY_LIFETIME_SECONDS)
    ):
        raise ReleaseError("trust registry lifetime is invalid")
    seen_ids: set[str] = set()
    seen_resources: set[str] = set()
    for field, purpose in (("releases", "release"), ("withdrawals", "withdrawal")):
        entries = value[field]
        if not isinstance(entries, list) or len(entries) > 32:
            raise ReleaseError(f"trust registry {field} count is invalid")
        for index, raw in enumerate(entries):
            entry = _validate_key_entry(
                require_object(raw, f"trust registry {field} {index}"),
                expected_purpose=purpose,
                label=f"trust registry {field} {index}",
            )
            if (
                entry["keyId"] in seen_ids
                or entry["kmsKeyVersion"] in seen_resources
            ):
                raise ReleaseError("trust registry authorities are not distinct")
            seen_ids.add(entry["keyId"])
            seen_resources.add(entry["kmsKeyVersion"])
    if signed:
        signatures = value["signatures"]
        if not isinstance(signatures, list) or not 2 <= len(signatures) <= 3:
            raise ReleaseError("trust registry requires two or three root signatures")
        signer_ids: set[str] = set()
        for index, raw in enumerate(signatures):
            signature = require_object(raw, f"trust registry signature {index}")
            exact_keys(
                signature,
                {"schema", "algorithm", "keyId", "payloadDigest", "signature"},
                f"trust registry signature {index}",
            )
            if (
                signature["schema"] != TRUST_REGISTRY_SCHEMA
                or signature["algorithm"] != "ed25519"
            ):
                raise ReleaseError("trust registry root signature is unsupported")
            key_id = require_match(
                signature["keyId"], ROOT_KEY_ID, "trust registry root signer id"
            )
            if key_id in signer_ids:
                raise ReleaseError("trust registry contains duplicate root signatures")
            signer_ids.add(key_id)
            if require_digest(
                signature["payloadDigest"], "trust registry signature digest"
            ) != registry_payload_digest(value):
                raise ReleaseError("trust registry signature digest differs")
            encoded = require_ascii(
                signature["signature"], "trust registry root signature", 86
            )
            if not re.fullmatch(r"[A-Za-z0-9_-]{86}", encoded):
                raise ReleaseError("trust registry signature is not canonical base64url")
    canonical_registry_payload(value)
    return value


def find_key(
    registry: dict[str, Any],
    key_id: str,
    *,
    purpose: str,
) -> dict[str, Any]:
    validate_registry(registry, signed="signatures" in registry)
    field = "releases" if purpose == "release" else "withdrawals"
    matches = [entry for entry in registry[field] if entry["keyId"] == key_id]
    if len(matches) != 1:
        raise ReleaseError(f"trust registry has no unique {purpose} key")
    return matches[0]
