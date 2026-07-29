#!/usr/bin/env python3
"""Build and verify website-compatible detached Cog release signatures."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

from cog_integrations import (
    ManifestValidationError,
    canonical_bytes as canonical_integration_bytes,
    validate_normalized_manifest,
)

PROVENANCE_SCHEMA = "cognitum.cog.release-provenance.v1"
WITHDRAWAL_SCHEMA = "cognitum.cog.release-withdrawal.v1"
TRUST_SCHEMA = "cognitum.cog.release-trust.v2"
EVIDENCE_LOCATIONS_SCHEMA = "cognitum.cog.release-evidence-locations.v1"
POLICY_SCHEMA = "cognitum.cog.release-policy.v1"
MAX_JSON_BYTES = 256 * 1024
MAX_CANONICAL_BYTES = 64 * 1024
MAX_VALUES = 2_048
MAX_DEPTH = 12
DIGEST = re.compile(r"^sha256:[a-f0-9]{64}$")
KEY_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")
COG_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$")
COMMIT = re.compile(r"^[a-f0-9]{40,64}$")
BUILT_AT = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
NUMERIC_GITHUB_ID = re.compile(r"^[1-9][0-9]{4,24}$")
KMS_KEY_VERSION = re.compile(
    r"^projects/[a-z][a-z0-9-]{4,28}[a-z0-9]/locations/[a-z0-9-]+/"
    r"keyRings/[A-Za-z0-9_-]{1,63}/cryptoKeys/[A-Za-z0-9_-]{1,63}/"
    r"cryptoKeyVersions/[1-9][0-9]*$"
)
REASON_CODE = re.compile(r"^[a-z][a-z0-9.-]{2,63}$")
RFC3339 = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?"
    r"(?:Z|[+-]\d{2}:\d{2})$"
)
BUCKET_NAME = re.compile(r"^[a-z0-9][a-z0-9._-]{1,61}[a-z0-9]$")
GENERATION = re.compile(r"^[1-9][0-9]{0,19}$")
MAX_EVIDENCE_OBJECTS = 256
MAX_EVIDENCE_RETENTION_SECONDS = 365 * 24 * 60 * 60
FIXED_DECLARATIONS = {
    "runtimeContractVersion": "cognitum.cog.v1",
    "packaging": "edge-cli-binary",
    "deploymentDriver": "edge-dispatch",
    "tenancyMode": "single-tenant-device",
}


class ReleaseError(ValueError):
    """A release input is unsafe, ambiguous, or incompatible."""


def exact_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = set(value) - allowed
    missing = allowed - set(value)
    if unknown or missing:
        raise ReleaseError(
            f"{label} fields differ: missing={sorted(missing)}, unknown={sorted(unknown)}"
        )


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReleaseError(f"{label} must be an object")
    return value


def require_ascii(value: Any, label: str, maximum: int = 512) -> str:
    if not isinstance(value, str) or not 1 <= len(value) <= maximum:
        raise ReleaseError(f"{label} must be a non-empty bounded string")
    if any(ord(character) < 0x20 or ord(character) > 0x7E for character in value):
        raise ReleaseError(f"{label} must contain printable ASCII only")
    return value


def require_match(value: Any, pattern: re.Pattern[str], label: str) -> str:
    text = require_ascii(value, label)
    if not pattern.fullmatch(text):
        raise ReleaseError(f"{label} is malformed")
    return text


def require_digest(value: Any, label: str) -> str:
    return require_match(value, DIGEST, label)


def require_strings(
    value: Any, label: str, maximum: int = 32, minimum: int = 1
) -> list[str]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ReleaseError(f"{label} must contain {minimum}-{maximum} strings")
    strings = [require_ascii(item, f"{label} entry") for item in value]
    if len(set(strings)) != len(strings):
        raise ReleaseError(f"{label} must not contain duplicates")
    return strings


def read_json(path: Path) -> dict[str, Any]:
    if not path.is_file() or path.stat().st_size > MAX_JSON_BYTES:
        raise ReleaseError(f"{path} is missing or too large")
    try:
        return require_object(json.loads(path.read_text(encoding="utf-8")), str(path))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseError(f"{path} is not valid UTF-8 JSON") from error


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False, allow_nan=False)
        + "\n",
        encoding="utf-8",
    )


def digest_file(path: Path) -> str:
    if not path.is_file():
        raise ReleaseError(f"evidence file does not exist: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return f"sha256:{digest.hexdigest()}"


def _canonical_check(value: Any, state: dict[str, int], depth: int = 0) -> None:
    state["values"] += 1
    if state["values"] > MAX_VALUES or depth > MAX_DEPTH:
        raise ReleaseError("release exceeds canonicalization limits")
    if value is None or isinstance(value, bool):
        return
    if (
        isinstance(value, int)
        and not isinstance(value, bool)
        and -(2**53 - 1) <= value <= 2**53 - 1
    ):
        return
    if isinstance(value, str):
        require_ascii(value, "canonical release string", 8_192)
        return
    if isinstance(value, list):
        if len(value) > 256:
            raise ReleaseError("canonical release array is too large")
        for item in value:
            _canonical_check(item, state, depth + 1)
        return
    if isinstance(value, dict):
        if len(value) > 256:
            raise ReleaseError("canonical release object is too large")
        for key, item in value.items():
            require_ascii(key, "canonical release field", 128)
            _canonical_check(item, state, depth + 1)
        return
    raise ReleaseError(f"unsupported canonical release value: {type(value).__name__}")


def canonical_payload(release: dict[str, Any]) -> bytes:
    provenance = require_object(release.get("provenance"), "release.provenance").copy()
    provenance.pop("detachedSignature", None)
    unsigned = dict(release)
    unsigned.pop("seededAt", None)
    unsigned["provenance"] = provenance
    statement = {"schema": PROVENANCE_SCHEMA, "release": unsigned}
    _canonical_check(statement, {"values": 0})
    encoded = json.dumps(
        statement,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    if len(encoded) > MAX_CANONICAL_BYTES:
        raise ReleaseError("canonical release statement is too large")
    return encoded


def payload_digest(release: dict[str, Any]) -> str:
    return f"sha256:{hashlib.sha256(canonical_payload(release)).hexdigest()}"


def canonical_withdrawal_payload(withdrawal: dict[str, Any]) -> bytes:
    unsigned = dict(withdrawal)
    unsigned.pop("detachedSignature", None)
    unsigned.pop("seededAt", None)
    statement = {"schema": WITHDRAWAL_SCHEMA, "withdrawal": unsigned}
    _canonical_check(statement, {"values": 0})
    encoded = json.dumps(
        statement,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    if len(encoded) > MAX_CANONICAL_BYTES:
        raise ReleaseError("canonical release withdrawal is too large")
    return encoded


def withdrawal_payload_digest(withdrawal: dict[str, Any]) -> str:
    return (
        "sha256:"
        f"{hashlib.sha256(canonical_withdrawal_payload(withdrawal)).hexdigest()}"
    )


def require_rfc3339(value: Any, label: str) -> str:
    text = require_match(value, RFC3339, label)
    try:
        datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError as error:
        raise ReleaseError(f"{label} is not a valid RFC3339 timestamp") from error
    return text


def validate_runtime_integrations(
    value: Any,
    *,
    cog_id: str | None = None,
    version: str | None = None,
) -> dict[str, Any]:
    runtime = require_object(value, "runtimeIntegrations")
    exact_keys(
        runtime,
        {"manifest", "manifestDigest", "staticWebsiteBundleDigest"},
        "runtimeIntegrations",
    )
    try:
        manifest = validate_normalized_manifest(
            runtime["manifest"], Path("<runtimeIntegrations.manifest>")
        )
    except ManifestValidationError as error:
        detail = "; ".join(str(issue) for issue in error.issues)
        raise ReleaseError(
            f"runtime integration manifest is invalid: {detail}"
        ) from error
    manifest_digest = require_digest(
        runtime["manifestDigest"], "runtimeIntegrations.manifestDigest"
    )
    measured_digest = (
        f"sha256:{hashlib.sha256(canonical_integration_bytes(manifest)).hexdigest()}"
    )
    if manifest_digest != measured_digest:
        raise ReleaseError(
            "runtime integration manifestDigest does not match the exact normalized manifest"
        )
    if cog_id is not None and manifest["cog"]["id"] != cog_id:
        raise ReleaseError("runtime integration manifest cog id does not match release")
    if version is not None and manifest["cog"]["version"] != version:
        raise ReleaseError(
            "runtime integration manifest version does not match release"
        )

    website = manifest["integrations"]["website"]
    requires_bundle = (
        website["enabled"] is True and website["artifact"]["kind"] == "static-build"
    )
    bundle_digest = runtime["staticWebsiteBundleDigest"]
    if requires_bundle:
        require_digest(bundle_digest, "runtimeIntegrations.staticWebsiteBundleDigest")
    elif bundle_digest is not None:
        raise ReleaseError(
            "staticWebsiteBundleDigest must be null unless a static website is enabled"
        )
    return runtime


def validate_policy(value: dict[str, Any]) -> dict[str, Any]:
    exact_keys(
        value,
        {
            "schema",
            "cogId",
            "blueprintId",
            "blueprintDigest",
            "runtimeContractVersion",
            "packaging",
            "deploymentDriver",
            "artifacts",
            "tenancyMode",
            "statePolicy",
            "stateSchemaVersion",
            "rollbackCompatibility",
            "networkPolicy",
            "runtimeIntegrations",
            "residency",
            "lifecycle",
            "ratifiedBy",
        },
        "release policy",
    )
    if value["schema"] != POLICY_SCHEMA:
        raise ReleaseError("unsupported release policy schema")
    cog_id = require_match(value["cogId"], COG_ID, "policy.cogId")
    if require_match(value["blueprintId"], COG_ID, "policy.blueprintId") != cog_id:
        raise ReleaseError("policy blueprintId must equal cogId")
    validate_runtime_integrations(value["runtimeIntegrations"], cog_id=cog_id)
    require_digest(value["blueprintDigest"], "policy.blueprintDigest")
    for field in (
        "runtimeContractVersion",
        "packaging",
        "deploymentDriver",
        "tenancyMode",
        "statePolicy",
        "stateSchemaVersion",
        "ratifiedBy",
    ):
        require_ascii(value[field], f"policy.{field}", 128)
    for field, expected in FIXED_DECLARATIONS.items():
        if value[field] != expected:
            raise ReleaseError(f"policy.{field} must be {expected}")
    if value["statePolicy"] not in {"none", "ephemeral", "persistent"}:
        raise ReleaseError("policy.statePolicy is unsupported")
    if value["lifecycle"] != "available":
        raise ReleaseError("release policy lifecycle must be available")
    artifacts = require_object(value["artifacts"], "policy.artifacts")
    if not artifacts or set(artifacts) - {"armhf", "aarch64"}:
        raise ReleaseError("policy artifacts must declare only armhf and/or aarch64")
    for arch, raw in artifacts.items():
        artifact = require_object(raw, f"policy.artifacts.{arch}")
        exact_keys(
            artifact, {"binaryName", "targetHardware"}, f"policy.artifacts.{arch}"
        )
        require_match(
            artifact["binaryName"], SAFE_NAME, f"policy artifact {arch} binaryName"
        )
        require_strings(
            artifact["targetHardware"], f"policy artifact {arch} targetHardware", 8
        )
    rollback = require_object(
        value["rollbackCompatibility"], "policy.rollbackCompatibility"
    )
    exact_keys(rollback, {"compatibleWith", "migrationsReversible"}, "rollback policy")
    if not isinstance(rollback["compatibleWith"], list):
        raise ReleaseError("rollback compatibleWith must be a list")
    for version in rollback["compatibleWith"]:
        require_match(version, VERSION, "rollback compatible version")
    if not isinstance(rollback["migrationsReversible"], bool):
        raise ReleaseError("migrationsReversible must be boolean")
    network = require_object(value["networkPolicy"], "policy.networkPolicy")
    exact_keys(network, {"egressPolicy", "egressAllowlist"}, "network policy")
    if network["egressPolicy"] not in {"deny-all", "allowlist"}:
        raise ReleaseError("network egressPolicy is unsupported")
    allowlist = require_strings(
        network["egressAllowlist"], "network egressAllowlist", minimum=0
    )
    if network["egressPolicy"] == "deny-all" and allowlist:
        raise ReleaseError("deny-all cannot have an egress allowlist")
    residency = require_object(value["residency"], "policy.residency")
    exact_keys(residency, {"allowedRegions", "dataResidency"}, "residency")
    require_strings(residency["allowedRegions"], "residency allowedRegions", 32)
    require_ascii(residency["dataResidency"], "residency dataResidency")
    return value


def validate_release(value: dict[str, Any], signed: bool) -> dict[str, Any]:
    exact_keys(
        value,
        {
            "cogId",
            "blueprintId",
            "blueprintDigest",
            "version",
            "releaseDigest",
            "sourceCommit",
            "runtimeContractVersion",
            "packaging",
            "deploymentDriver",
            "artifactRef",
            "tenancyMode",
            "statePolicy",
            "stateSchemaVersion",
            "rollbackCompatibility",
            "networkPolicy",
            "runtimeIntegrations",
            "provenance",
            "securityAttestation",
            "residency",
            "lifecycle",
        },
        "release",
    )
    cog_id = require_match(value["cogId"], COG_ID, "release.cogId")
    if require_match(value["blueprintId"], COG_ID, "release.blueprintId") != cog_id:
        raise ReleaseError("release blueprintId must equal cogId")
    require_digest(value["blueprintDigest"], "release.blueprintDigest")
    release_version = require_match(value["version"], VERSION, "release.version")
    require_digest(value["releaseDigest"], "release.releaseDigest")
    require_match(value["sourceCommit"], COMMIT, "release.sourceCommit")
    for field in (
        "runtimeContractVersion",
        "packaging",
        "deploymentDriver",
        "tenancyMode",
        "statePolicy",
        "stateSchemaVersion",
    ):
        require_ascii(value[field], f"release.{field}", 128)
    for field, expected in FIXED_DECLARATIONS.items():
        if value[field] != expected:
            raise ReleaseError(f"release.{field} must be {expected}")
    if value["statePolicy"] not in {"none", "ephemeral", "persistent"}:
        raise ReleaseError("release.statePolicy is unsupported")
    if value["lifecycle"] != "available":
        raise ReleaseError("release lifecycle must be available")
    validate_runtime_integrations(
        value["runtimeIntegrations"], cog_id=cog_id, version=release_version
    )
    rollback = require_object(value["rollbackCompatibility"], "rollbackCompatibility")
    exact_keys(
        rollback, {"compatibleWith", "migrationsReversible"}, "rollbackCompatibility"
    )
    if not isinstance(rollback["compatibleWith"], list) or not isinstance(
        rollback["migrationsReversible"], bool
    ):
        raise ReleaseError("release rollbackCompatibility is malformed")
    for compatible in rollback["compatibleWith"]:
        require_match(compatible, VERSION, "rollback compatible version")
    network = require_object(value["networkPolicy"], "networkPolicy")
    exact_keys(network, {"egressPolicy", "egressAllowlist"}, "networkPolicy")
    if network["egressPolicy"] not in {"deny-all", "allowlist"}:
        raise ReleaseError("release network egressPolicy is unsupported")
    allowlist = require_strings(
        network["egressAllowlist"], "release egressAllowlist", minimum=0
    )
    if network["egressPolicy"] == "deny-all" and allowlist:
        raise ReleaseError("release deny-all policy cannot have an allowlist")
    residency = require_object(value["residency"], "residency")
    exact_keys(residency, {"allowedRegions", "dataResidency"}, "residency")
    require_strings(residency["allowedRegions"], "release allowedRegions")
    require_ascii(residency["dataResidency"], "release dataResidency")
    artifact = require_object(value["artifactRef"], "release.artifactRef")
    exact_keys(
        artifact,
        {"kind", "binaryDigest", "binaryName", "targetHardware"},
        "artifactRef",
    )
    if artifact["kind"] != "edge-binary":
        raise ReleaseError("release artifact kind must be edge-binary")
    binary_digest = require_digest(artifact["binaryDigest"], "artifactRef.binaryDigest")
    if binary_digest != value["releaseDigest"]:
        raise ReleaseError("artifactRef binaryDigest must equal releaseDigest")
    require_match(artifact["binaryName"], SAFE_NAME, "artifactRef.binaryName")
    require_strings(artifact["targetHardware"], "artifactRef.targetHardware", 8)
    provenance = require_object(value["provenance"], "release.provenance")
    provenance_keys = {
        "signatureAlgorithm",
        "signingKeyId",
        "builderIdentity",
        "buildWorkflow",
        "dependencyLockDigest",
        "sbomDigest",
        "provenanceDigest",
        "builtAt",
    }
    if signed:
        provenance_keys.add("detachedSignature")
    exact_keys(provenance, provenance_keys, "release.provenance")
    if provenance["signatureAlgorithm"] != "ed25519":
        raise ReleaseError("release signatureAlgorithm must be ed25519")
    require_match(provenance["signingKeyId"], KEY_ID, "release signingKeyId")
    require_ascii(provenance["builderIdentity"], "release builderIdentity")
    require_ascii(provenance["buildWorkflow"], "release buildWorkflow")
    require_match(provenance["builtAt"], BUILT_AT, "release builtAt")
    for field in ("dependencyLockDigest", "sbomDigest", "provenanceDigest"):
        require_digest(provenance[field], f"release provenance {field}")
    security = require_object(value["securityAttestation"], "securityAttestation")
    exact_keys(
        security,
        {
            "vulnerabilityScanDigest",
            "policyDecisionDigest",
            "isolationEvidenceDigest",
            "isolationPassed",
        },
        "securityAttestation",
    )
    for field in (
        "vulnerabilityScanDigest",
        "policyDecisionDigest",
        "isolationEvidenceDigest",
    ):
        require_digest(security[field], f"securityAttestation.{field}")
    if security["isolationPassed"] is not True:
        raise ReleaseError("release isolationPassed must be true")
    if signed:
        envelope = require_object(provenance["detachedSignature"], "detachedSignature")
        exact_keys(
            envelope,
            {"schema", "algorithm", "keyId", "payloadDigest", "signature"},
            "detachedSignature",
        )
        if (
            envelope["schema"] != PROVENANCE_SCHEMA
            or envelope["algorithm"] != "ed25519"
        ):
            raise ReleaseError("detached signature schema or algorithm is unsupported")
        if envelope["keyId"] != provenance["signingKeyId"]:
            raise ReleaseError("detached signature key id is not bound to provenance")
        require_digest(envelope["payloadDigest"], "detachedSignature.payloadDigest")
        signature = require_ascii(
            envelope["signature"], "detachedSignature.signature", 86
        )
        if len(signature) != 86 or not re.fullmatch(r"[A-Za-z0-9_-]{86}", signature):
            raise ReleaseError(
                "detached signature must be canonical unpadded base64url"
            )
        if envelope["payloadDigest"] != payload_digest(value):
            raise ReleaseError(
                "detached signature payload digest does not match release"
            )
    canonical_payload(value)
    return value


def validate_withdrawal(
    value: dict[str, Any],
    release: dict[str, Any],
    *,
    signed: bool,
) -> dict[str, Any]:
    allowed = {
        "schema",
        "releaseDigest",
        "releasePayloadDigest",
        "cogId",
        "action",
        "reasonCode",
        "issuedAt",
        "issuer",
    }
    if signed:
        allowed.add("detachedSignature")
        if "seededAt" in value:
            allowed.add("seededAt")
    exact_keys(value, allowed, "release withdrawal")
    if value["schema"] != WITHDRAWAL_SCHEMA:
        raise ReleaseError("unsupported release withdrawal schema")

    validate_release(release, signed=True)
    if value["releaseDigest"] != release["releaseDigest"]:
        raise ReleaseError("withdrawal releaseDigest is not bound to the release")
    require_digest(value["releaseDigest"], "withdrawal.releaseDigest")
    release_envelope = require_object(
        require_object(release["provenance"], "release.provenance")[
            "detachedSignature"
        ],
        "release detached signature",
    )
    if value["releasePayloadDigest"] != release_envelope["payloadDigest"]:
        raise ReleaseError(
            "withdrawal releasePayloadDigest is not bound to the signed release"
        )
    require_digest(
        value["releasePayloadDigest"], "withdrawal.releasePayloadDigest"
    )
    if value["cogId"] != release["cogId"]:
        raise ReleaseError("withdrawal cogId is not bound to the release")
    require_match(value["cogId"], COG_ID, "withdrawal.cogId")
    if value["action"] not in {"withdrawn", "revoked"}:
        raise ReleaseError("withdrawal action must be withdrawn or revoked")
    require_match(value["reasonCode"], REASON_CODE, "withdrawal.reasonCode")
    require_rfc3339(value["issuedAt"], "withdrawal.issuedAt")
    if "seededAt" in value:
        require_rfc3339(value["seededAt"], "withdrawal.seededAt")

    issuer = require_object(value["issuer"], "withdrawal.issuer")
    exact_keys(
        issuer,
        {
            "identity",
            "workflow",
            "githubOwnerId",
            "githubRepositoryId",
            "githubWorkflowId",
        },
        "withdrawal.issuer",
    )
    require_ascii(issuer["identity"], "withdrawal issuer identity")
    require_ascii(issuer["workflow"], "withdrawal issuer workflow")
    for field in (
        "githubOwnerId",
        "githubRepositoryId",
        "githubWorkflowId",
    ):
        require_match(
            issuer[field],
            NUMERIC_GITHUB_ID,
            f"withdrawal issuer {field}",
        )

    if signed:
        envelope = require_object(
            value["detachedSignature"], "withdrawal.detachedSignature"
        )
        exact_keys(
            envelope,
            {"schema", "algorithm", "keyId", "payloadDigest", "signature"},
            "withdrawal.detachedSignature",
        )
        if (
            envelope["schema"] != WITHDRAWAL_SCHEMA
            or envelope["algorithm"] != "ed25519"
        ):
            raise ReleaseError(
                "withdrawal detached signature schema or algorithm is unsupported"
            )
        require_match(
            envelope["keyId"], KEY_ID, "withdrawal detached signature key id"
        )
        require_digest(
            envelope["payloadDigest"],
            "withdrawal detached signature payloadDigest",
        )
        signature = require_ascii(
            envelope["signature"],
            "withdrawal detached signature",
            86,
        )
        if len(signature) != 86 or not re.fullmatch(
            r"[A-Za-z0-9_-]{86}", signature
        ):
            raise ReleaseError(
                "withdrawal signature must be canonical unpadded base64url"
            )
        if envelope["payloadDigest"] != withdrawal_payload_digest(value):
            raise ReleaseError(
                "withdrawal payload digest does not match the canonical statement"
            )
    canonical_withdrawal_payload(value)
    return value


def _evidence_path_pattern(bucket: str, kind: str) -> re.Pattern[str]:
    escaped = re.escape(bucket)
    if kind == "release":
        return re.compile(
            rf"^gs://{escaped}/staging/cogs/releases/"
            r"[a-z0-9]+(?:-[a-z0-9]+)*/\d+\.\d+\.\d+/"
            r"(?:armhf|aarch64)/evidence/sha256/([a-f0-9]{64})/"
            r"release-evidence\.json$"
        )
    return re.compile(
        rf"^gs://{escaped}/staging/cogs/withdrawals/sha256/"
        r"([a-f0-9]{64})/evidence/sha256/([a-f0-9]{64})/"
        r"release-withdrawal\.json$"
    )


def validate_evidence_locations(value: dict[str, Any]) -> dict[str, Any]:
    exact_keys(value, {"schema", "bucket", "objects"}, "evidence locations")
    if value["schema"] != EVIDENCE_LOCATIONS_SCHEMA:
        raise ReleaseError("unsupported release evidence locations schema")
    bucket = require_object(value["bucket"], "evidence locations bucket")
    exact_keys(
        bucket,
        {
            "name",
            "resource",
            "publicAccessPrevention",
            "uniformBucketLevelAccess",
            "retentionPeriodSeconds",
            "retentionPolicyLocked",
            "versioningEnabled",
        },
        "evidence locations bucket",
    )
    name = require_match(bucket["name"], BUCKET_NAME, "evidence bucket name")
    if name == "cognitum-apps":
        raise ReleaseError("release evidence must use a dedicated protected bucket")
    if bucket["resource"] != (
        f"//storage.googleapis.com/projects/_/buckets/{name}"
    ):
        raise ReleaseError("release evidence bucket resource does not match")
    if (
        bucket["publicAccessPrevention"] != "enforced"
        or bucket["uniformBucketLevelAccess"] is not True
        or bucket["versioningEnabled"] is not True
    ):
        raise ReleaseError("release evidence bucket protections are incomplete")
    retention = bucket["retentionPeriodSeconds"]
    if (
        not isinstance(retention, int)
        or isinstance(retention, bool)
        or not 1 <= retention <= MAX_EVIDENCE_RETENTION_SECONDS
    ):
        raise ReleaseError("release evidence retention period is invalid")
    if not isinstance(bucket["retentionPolicyLocked"], bool):
        raise ReleaseError("release evidence retention lock state must be boolean")

    objects = value["objects"]
    if not isinstance(objects, list) or len(objects) > MAX_EVIDENCE_OBJECTS:
        raise ReleaseError("release evidence objects are invalid")
    seen: set[tuple[str, str, str]] = set()
    for index, raw in enumerate(objects):
        entry = require_object(raw, f"evidence object {index}")
        exact_keys(
            entry,
            {"kind", "uri", "generation", "contentDigest", "ifGenerationMatch"},
            f"evidence object {index}",
        )
        kind = entry["kind"]
        if kind not in {"release", "withdrawal"}:
            raise ReleaseError(f"evidence object {index} kind is unsupported")
        uri = require_ascii(entry["uri"], f"evidence object {index} URI", 1_024)
        match = _evidence_path_pattern(name, kind).fullmatch(uri)
        if not match:
            raise ReleaseError(
                f"evidence object {index} URI is not content addressed"
            )
        content_hex = match.group(2 if kind == "withdrawal" else 1)
        if entry["contentDigest"] != f"sha256:{content_hex}":
            raise ReleaseError(
                f"evidence object {index} digest does not match its URI"
            )
        require_digest(
            entry["contentDigest"], f"evidence object {index} contentDigest"
        )
        generation = require_match(
            entry["generation"], GENERATION, f"evidence object {index} generation"
        )
        if entry["ifGenerationMatch"] != 0:
            raise ReleaseError(
                f"evidence object {index} is missing if-generation-match=0"
            )
        identity = (kind, uri, generation)
        if identity in seen:
            raise ReleaseError("release evidence locations contain duplicates")
        seen.add(identity)
    return value
