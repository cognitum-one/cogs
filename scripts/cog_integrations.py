#!/usr/bin/env python3
"""Validate cog.toml runtime integrations and emit a canonical v1 manifest.

The source contract is intentionally declarative. It accepts no commands,
credentials, Secret Manager resource names, observed URLs, or deployment
status. Missing integration tables normalize to explicit disabled records.
"""
from __future__ import annotations

import hashlib
import json
import re
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

SCHEMA_VERSION = "cognitum.cog.integrations.v1"
MCP_PROTOCOL_VERSION = "2025-11-25"
BUILD_PROFILE = "vite-production-v1"
TAILSCALE_BINDING = "tailscale-oauth-client"
COGS_DIR = Path(__file__).resolve().parent.parent / "src" / "cogs"

ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
APPROVAL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$")
NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")
TAILNET_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9.-]{1,252}[A-Za-z0-9]$")
HOSTNAME_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$")
TAG_RE = re.compile(r"^tag:[a-z0-9][a-z0-9-]{0,62}$")
OCI_RE = re.compile(r"^[^\s@]+(?:/[^\s@]+)*@sha256:[a-f0-9]{64}$")
FORBIDDEN_MCP_AUTHORITY = re.compile(
    r"(^|[._:-])(deploy|billing|iam|network|tailscale|secret|approve|release)"
    r"([._:-]|$)",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class Issue:
    path: str
    code: str
    message: str

    def __str__(self) -> str:
        return f"{self.path}: {self.code}: {self.message}"


class ManifestValidationError(ValueError):
    def __init__(self, path: Path, issues: list[Issue]):
        super().__init__(f"{path}: {len(issues)} integration manifest issue(s)")
        self.path = path
        self.issues = issues


def _issue(issues: list[Issue], path: str, code: str, message: str) -> None:
    issues.append(Issue(path, code, message))


def _table(value: Any, path: str, issues: list[Issue]) -> dict[str, Any]:
    if not isinstance(value, dict):
        _issue(issues, path, "type", "must be a table")
        return {}
    return value


def _strict(table: dict[str, Any], allowed: set[str], path: str, issues: list[Issue]) -> None:
    for key in sorted(set(table) - allowed):
        _issue(issues, f"{path}.{key}", "unknown-field", "field is not allowed")


def _required(table: dict[str, Any], key: str, path: str, issues: list[Issue]) -> Any:
    if key not in table:
        _issue(issues, f"{path}.{key}", "required", "field is required")
        return None
    return table[key]


def _integer(value: Any, minimum: int, maximum: int) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and minimum <= value <= maximum


def _safe_path(value: Any) -> bool:
    return (
        isinstance(value, str)
        and 0 < len(value) <= 256
        and value.startswith("/")
        and "\\" not in value
        and "//" not in value
        and all(part not in {".", ".."} for part in value.split("/"))
    )


def _safe_relative(value: Any) -> bool:
    return (
        isinstance(value, str)
        and 0 < len(value) <= 256
        and not value.startswith("/")
        and "\\" not in value
        and all(part not in {"", ".", ".."} for part in value.split("/"))
    )


def _https_url(value: Any, exact_origin: bool = False) -> bool:
    if not isinstance(value, str) or len(value) > 2048:
        return False
    try:
        parsed = urlsplit(value)
    except ValueError:
        return False
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username
        or parsed.password
        or parsed.fragment
    ):
        return False
    if exact_origin and (parsed.path or parsed.query):
        return False
    return True


def _string_list(
    value: Any,
    path: str,
    issues: list[Issue],
    pattern: re.Pattern[str],
    maximum: int,
) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or len(value) > maximum
        or any(not isinstance(item, str) or not pattern.fullmatch(item) for item in value)
    ):
        _issue(issues, path, "invalid-list", f"must be 1-{maximum} valid unique strings")
        return []
    if len(set(value)) != len(value):
        _issue(issues, path, "duplicate", "values must be unique")
    return value


def _health(raw: Any, path: str, issues: list[Issue]) -> dict[str, Any]:
    value = _table(raw, path, issues)
    _strict(value, {"path", "interval_seconds", "timeout_seconds"}, path, issues)
    health_path = _required(value, "path", path, issues)
    interval = _required(value, "interval_seconds", path, issues)
    timeout = _required(value, "timeout_seconds", path, issues)
    if not _safe_path(health_path):
        _issue(issues, f"{path}.path", "unsafe-path", "must be a normalized absolute path")
    if not _integer(interval, 5, 300):
        _issue(issues, f"{path}.interval_seconds", "range", "must be 5-300")
    if not _integer(timeout, 1, 30) or (
        _integer(interval, 5, 300) and isinstance(timeout, int) and timeout >= interval
    ):
        _issue(issues, f"{path}.timeout_seconds", "range", "must be 1-30 and shorter than interval")
    return {"path": health_path, "intervalSeconds": interval, "timeoutSeconds": timeout}


def _approval(raw: Any, path: str, issues: list[Issue]) -> dict[str, Any]:
    value = _table(raw, path, issues)
    _strict(value, {"approved", "approval_ref"}, path, issues)
    approved = _required(value, "approved", path, issues)
    approval_ref = _required(value, "approval_ref", path, issues)
    if approved is not True:
        _issue(issues, f"{path}.approved", "approval-required", "must be true")
    if not isinstance(approval_ref, str) or not APPROVAL_RE.fullmatch(approval_ref):
        _issue(issues, f"{path}.approval_ref", "invalid-reference", "must be an immutable approval reference")
    return {"approved": approved, "approvalRef": approval_ref}


def _exposure(
    source: dict[str, Any],
    path: str,
    issues: list[Issue],
) -> tuple[Any, Any, dict[str, Any] | None]:
    exposure = _required(source, "exposure", path, issues)
    ingress = _required(source, "ingress", path, issues)
    if exposure not in {"private", "public"}:
        _issue(issues, f"{path}.exposure", "invalid-exposure", "must be private or public")
    if ingress not in {"internal", "tailscale", "public"}:
        _issue(issues, f"{path}.ingress", "invalid-ingress", "must be internal, tailscale, or public")
    approval = None
    if "public_access_approval" in source:
        approval = _approval(source["public_access_approval"], f"{path}.public_access_approval", issues)
    if exposure == "private" and ingress == "public":
        _issue(issues, f"{path}.ingress", "implicit-public", "private interfaces cannot use public ingress")
    if exposure == "public":
        if ingress != "public":
            _issue(issues, f"{path}.ingress", "public-ingress-required", "public interfaces require public ingress")
        if approval is None:
            _issue(issues, f"{path}.public_access_approval", "approval-required", "public exposure requires approval")
    return exposure, ingress, approval


def _website(raw: Any, issues: list[Issue]) -> dict[str, Any]:
    path = "integrations.website"
    value = _table(raw, path, issues)
    allowed = {
        "enabled", "artifact", "port", "base_path", "health_check", "auth",
        "exposure", "ingress", "public_access_approval",
    }
    _strict(value, allowed, path, issues)
    enabled = _required(value, "enabled", path, issues)
    if enabled is False:
        _strict(value, {"enabled"}, path, issues)
        return {"enabled": False}
    if enabled is not True:
        _issue(issues, f"{path}.enabled", "type", "must be a boolean")

    artifact_path = f"{path}.artifact"
    artifact = _table(_required(value, "artifact", path, issues), artifact_path, issues)
    kind = _required(artifact, "kind", artifact_path, issues)
    if kind == "static-build":
        _strict(artifact, {"kind", "build_profile", "output_directory"}, artifact_path, issues)
        profile = _required(artifact, "build_profile", artifact_path, issues)
        output = _required(artifact, "output_directory", artifact_path, issues)
        if profile != BUILD_PROFILE:
            _issue(issues, f"{artifact_path}.build_profile", "profile-required", f"must be {BUILD_PROFILE}")
        if not _safe_relative(output):
            _issue(issues, f"{artifact_path}.output_directory", "unsafe-path", "must be repository-relative")
        normalized_artifact = {"kind": kind, "buildProfile": profile, "outputDirectory": output}
    elif kind == "oci-image":
        _strict(artifact, {"kind", "image"}, artifact_path, issues)
        image = _required(artifact, "image", artifact_path, issues)
        if not isinstance(image, str) or not OCI_RE.fullmatch(image):
            _issue(issues, f"{artifact_path}.image", "immutable-image-required", "must be pinned by sha256 digest")
        normalized_artifact = {"kind": kind, "image": image}
    else:
        _strict(artifact, {"kind"}, artifact_path, issues)
        _issue(issues, f"{artifact_path}.kind", "invalid-kind", "must be static-build or oci-image")
        normalized_artifact = {"kind": kind}

    port = _required(value, "port", path, issues)
    base_path = _required(value, "base_path", path, issues)
    if not _integer(port, 1, 65535):
        _issue(issues, f"{path}.port", "range", "must be 1-65535")
    if not _safe_path(base_path):
        _issue(issues, f"{path}.base_path", "unsafe-path", "must be a normalized absolute path")
    health = _health(_required(value, "health_check", path, issues), f"{path}.health_check", issues)

    auth_path = f"{path}.auth"
    auth = _table(_required(value, "auth", path, issues), auth_path, issues)
    mode = _required(auth, "mode", auth_path, issues)
    if mode == "tenant-session":
        _strict(auth, {"mode", "audience"}, auth_path, issues)
        audience = _required(auth, "audience", auth_path, issues)
        normalized_auth = {"mode": mode, "audience": audience}
    elif mode == "oidc":
        _strict(auth, {"mode", "issuer", "audience"}, auth_path, issues)
        issuer = _required(auth, "issuer", auth_path, issues)
        audience = _required(auth, "audience", auth_path, issues)
        if not _https_url(issuer):
            _issue(issues, f"{auth_path}.issuer", "https-required", "must be an HTTPS issuer URL")
        normalized_auth = {"mode": mode, "issuer": issuer, "audience": audience}
    elif mode == "none":
        _strict(auth, {"mode"}, auth_path, issues)
        normalized_auth = {"mode": mode}
    else:
        _strict(auth, {"mode"}, auth_path, issues)
        _issue(issues, f"{auth_path}.mode", "invalid-auth", "must be tenant-session, oidc, or none")
        normalized_auth = {"mode": mode}
    if mode in {"tenant-session", "oidc"} and (
        not isinstance(audience, str) or not 1 <= len(audience) <= 256
    ):
        _issue(issues, f"{auth_path}.audience", "invalid-audience", "must be 1-256 characters")

    exposure, ingress, approval = _exposure(value, path, issues)
    if mode == "none" and exposure != "public":
        _issue(issues, f"{auth_path}.mode", "auth-required", "unauthenticated websites must be approved public interfaces")
    normalized = {
        "enabled": True, "artifact": normalized_artifact, "port": port,
        "basePath": base_path, "healthCheck": health, "auth": normalized_auth,
        "exposure": exposure, "ingress": ingress,
    }
    if approval is not None:
        normalized["publicAccessApproval"] = approval
    return normalized


def _tailscale(raw: Any, issues: list[Issue]) -> dict[str, Any]:
    path = "integrations.tailscale"
    value = _table(raw, path, issues)
    allowed = {
        "enabled", "tailnet", "hostname", "tags", "ephemeral",
        "key_expiry_seconds", "credential_binding",
    }
    _strict(value, allowed, path, issues)
    enabled = _required(value, "enabled", path, issues)
    if enabled is False:
        _strict(value, {"enabled"}, path, issues)
        return {"enabled": False}
    if enabled is not True:
        _issue(issues, f"{path}.enabled", "type", "must be a boolean")
    tailnet = _required(value, "tailnet", path, issues)
    hostname = _required(value, "hostname", path, issues)
    tags = _required(value, "tags", path, issues)
    ephemeral = _required(value, "ephemeral", path, issues)
    expiry = _required(value, "key_expiry_seconds", path, issues)
    binding = _required(value, "credential_binding", path, issues)
    if not isinstance(tailnet, str) or not TAILNET_RE.fullmatch(tailnet):
        _issue(issues, f"{path}.tailnet", "invalid-tailnet", "must be an approved tailnet DNS name")
    if not isinstance(hostname, str) or not HOSTNAME_RE.fullmatch(hostname):
        _issue(issues, f"{path}.hostname", "invalid-hostname", "must be a lowercase DNS label")
    normalized_tags = _string_list(tags, f"{path}.tags", issues, TAG_RE, 16)
    if ephemeral is not True:
        _issue(issues, f"{path}.ephemeral", "ephemeral-required", "must be true")
    if not _integer(expiry, 300, 86400):
        _issue(issues, f"{path}.key_expiry_seconds", "range", "must be 300-86400")
    if binding != TAILSCALE_BINDING:
        _issue(issues, f"{path}.credential_binding", "server-binding-required", f"must be {TAILSCALE_BINDING}")
    return {
        "enabled": True, "tailnet": tailnet, "hostname": hostname,
        "tags": normalized_tags, "ephemeral": ephemeral,
        "keyExpirySeconds": expiry, "credentialBinding": binding,
    }


def _web_mcp(raw: Any, issues: list[Issue]) -> dict[str, Any]:
    path = "integrations.web_mcp"
    value = _table(raw, path, issues)
    allowed = {
        "enabled", "transport", "protocol_version", "legacy_sse_acknowledged",
        "endpoint_path", "health_check", "auth", "allowed_tools", "scopes",
        "cors", "rate_limit", "exposure", "ingress", "public_access_approval",
    }
    _strict(value, allowed, path, issues)
    enabled = _required(value, "enabled", path, issues)
    if enabled is False:
        _strict(value, {"enabled"}, path, issues)
        return {"enabled": False}
    if enabled is not True:
        _issue(issues, f"{path}.enabled", "type", "must be a boolean")
    transport = value.get("transport", "streamable-http")
    protocol = value.get("protocol_version", MCP_PROTOCOL_VERSION)
    if transport not in {"streamable-http", "legacy-sse"}:
        _issue(issues, f"{path}.transport", "invalid-transport", "must be streamable-http or legacy-sse")
    if protocol != MCP_PROTOCOL_VERSION:
        _issue(issues, f"{path}.protocol_version", "unsupported-version", f"must be {MCP_PROTOCOL_VERSION}")
    legacy_ack = value.get("legacy_sse_acknowledged")
    if transport == "legacy-sse" and legacy_ack is not True:
        _issue(issues, f"{path}.legacy_sse_acknowledged", "legacy-ack-required", "must be true for legacy SSE")
    if legacy_ack is not None and not isinstance(legacy_ack, bool):
        _issue(issues, f"{path}.legacy_sse_acknowledged", "type", "must be a boolean")
    endpoint = _required(value, "endpoint_path", path, issues)
    if not _safe_path(endpoint):
        _issue(issues, f"{path}.endpoint_path", "unsafe-path", "must be a normalized absolute path")
    health = _health(_required(value, "health_check", path, issues), f"{path}.health_check", issues)

    auth_path = f"{path}.auth"
    auth = _table(_required(value, "auth", path, issues), auth_path, issues)
    mode = _required(auth, "mode", auth_path, issues)
    if mode == "oauth2":
        _strict(auth, {"mode", "resource_metadata_url", "audience"}, auth_path, issues)
        metadata = _required(auth, "resource_metadata_url", auth_path, issues)
        audience = _required(auth, "audience", auth_path, issues)
        if not _https_url(metadata):
            _issue(issues, f"{auth_path}.resource_metadata_url", "https-required", "must be an HTTPS URL")
        normalized_auth = {"mode": mode, "resourceMetadataUrl": metadata, "audience": audience}
    elif mode == "tenant-token":
        _strict(auth, {"mode", "audience"}, auth_path, issues)
        audience = _required(auth, "audience", auth_path, issues)
        normalized_auth = {"mode": mode, "audience": audience}
    else:
        _strict(auth, {"mode"}, auth_path, issues)
        _issue(issues, f"{auth_path}.mode", "invalid-auth", "must be oauth2 or tenant-token")
        audience = None
        normalized_auth = {"mode": mode}
    if not isinstance(audience, str) or not 1 <= len(audience) <= 256:
        _issue(issues, f"{auth_path}.audience", "invalid-audience", "must be 1-256 characters")

    tools = _string_list(
        _required(value, "allowed_tools", path, issues),
        f"{path}.allowed_tools", issues, NAME_RE, 128,
    )
    for index, tool in enumerate(tools):
        if FORBIDDEN_MCP_AUTHORITY.search(tool):
            _issue(issues, f"{path}.allowed_tools.{index}", "forbidden-authority", "tool exceeds MCP authority")
    scopes = _string_list(
        _required(value, "scopes", path, issues),
        f"{path}.scopes", issues, NAME_RE, 64,
    )
    cors_path = f"{path}.cors"
    cors = _table(_required(value, "cors", path, issues), cors_path, issues)
    _strict(cors, {"allowed_origins"}, cors_path, issues)
    origins_raw = _required(cors, "allowed_origins", cors_path, issues)
    origins = origins_raw if isinstance(origins_raw, list) else []
    origins_are_strings = all(isinstance(origin, str) for origin in origins)
    if (
        not origins
        or len(origins) > 32
        or not origins_are_strings
        or (origins_are_strings and len(set(origins)) != len(origins))
        or any(not _https_url(origin, exact_origin=True) for origin in origins)
    ):
        _issue(issues, f"{cors_path}.allowed_origins", "invalid-origin", "must be 1-32 unique exact HTTPS origins")

    rate_path = f"{path}.rate_limit"
    rate = _table(_required(value, "rate_limit", path, issues), rate_path, issues)
    _strict(rate, {"requests_per_minute", "burst"}, rate_path, issues)
    rpm = _required(rate, "requests_per_minute", rate_path, issues)
    burst = _required(rate, "burst", rate_path, issues)
    if not _integer(rpm, 1, 600):
        _issue(issues, f"{rate_path}.requests_per_minute", "range", "must be 1-600")
    if not _integer(burst, 1, 100) or (
        _integer(rpm, 1, 600) and isinstance(burst, int) and burst > rpm
    ):
        _issue(issues, f"{rate_path}.burst", "range", "must be 1-100 and no greater than requests_per_minute")
    exposure, ingress, approval = _exposure(value, path, issues)
    if exposure == "public" and mode != "oauth2":
        _issue(issues, f"{auth_path}.mode", "oauth-required", "public MCP requires OAuth 2.1 protected-resource metadata")
    normalized = {
        "enabled": True, "transport": transport, "protocolVersion": protocol,
        "endpointPath": endpoint, "healthCheck": health, "auth": normalized_auth,
        "allowedTools": tools, "scopes": scopes, "cors": {"allowedOrigins": origins},
        "rateLimit": {"requestsPerMinute": rpm, "burst": burst},
        "exposure": exposure, "ingress": ingress,
    }
    if legacy_ack is not None:
        normalized["legacySseAcknowledged"] = legacy_ack
    if approval is not None:
        normalized["publicAccessApproval"] = approval
    return normalized


def normalize_manifest(data: dict[str, Any], source: Path = Path("<memory>")) -> dict[str, Any]:
    issues: list[Issue] = []
    cog = _table(data.get("cog"), "cog", issues)
    cog_id = cog.get("id")
    version = cog.get("version")
    if not isinstance(cog_id, str) or not ID_RE.fullmatch(cog_id):
        _issue(issues, "cog.id", "invalid-id", "must be a lowercase kebab-case cog id")
    if not isinstance(version, str) or not VERSION_RE.fullmatch(version):
        _issue(issues, "cog.version", "invalid-version", "must be MAJOR.MINOR.PATCH")
    raw_integrations = data.get("integrations", {})
    integrations = _table(raw_integrations, "integrations", issues)
    _strict(integrations, {"website", "tailscale", "web_mcp"}, "integrations", issues)
    normalized = {
        "schemaVersion": SCHEMA_VERSION,
        "cog": {"id": cog_id, "version": version},
        "integrations": {
            "website": _website(integrations.get("website", {"enabled": False}), issues),
            "tailscale": _tailscale(integrations.get("tailscale", {"enabled": False}), issues),
            "webMcp": _web_mcp(integrations.get("web_mcp", {"enabled": False}), issues),
        },
    }
    ingress_needs_tailscale = any(
        normalized["integrations"][name].get("enabled")
        and normalized["integrations"][name].get("ingress") == "tailscale"
        for name in ("website", "webMcp")
    )
    if ingress_needs_tailscale and not normalized["integrations"]["tailscale"].get("enabled"):
        _issue(issues, "integrations.tailscale.enabled", "tailscale-required", "tailscale ingress requires attachment")
    if issues:
        raise ManifestValidationError(source, issues)
    return normalized


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            data = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ManifestValidationError(path, [Issue("$", "parse", str(exc))]) from exc
    return normalize_manifest(data, path)


def canonical_bytes(manifest: dict[str, Any]) -> bytes:
    return (json.dumps(manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()


def emit_manifest(path: Path, output_dir: Path) -> tuple[Path, str, Path]:
    manifest = load_manifest(path)
    payload = canonical_bytes(manifest)
    digest = hashlib.sha256(payload).hexdigest()
    cog_id = manifest["cog"]["id"]
    name = f"cog-{cog_id}-integrations-v1-sha256-{digest}.json"
    output_dir.mkdir(parents=True, exist_ok=True)
    output = output_dir / name
    output.write_bytes(payload)
    checksum = output.with_suffix(output.suffix + ".sha256")
    checksum.write_text(f"{digest}  {name}\n", encoding="utf-8")
    return output, digest, checksum
