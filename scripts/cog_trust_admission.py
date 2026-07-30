#!/usr/bin/env python3
"""Fail-closed source verifier for ADR-156 trust-registry admission.

This module does not sign, publish, rotate federation, or update a runtime
pin.  It verifies an already quorum-signed source package and emits the exact
create-only append plan consumed by the protected staging workflow.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from cog_release_provenance import verify_trust_registry
from cog_release_provenance_lib import (
    KEY_ID,
    NUMERIC_GITHUB_ID,
    ReleaseError,
    read_json,
    require_digest,
    require_match,
    write_json,
)
from cog_trust_registry import (
    GENESIS,
    bootstrap_digest,
    registry_payload_digest,
    validate_bootstrap,
    validate_registry,
)

SOURCE_ROOT = Path(__file__).resolve().parents[1]
BOOTSTRAP_RELATIVE_PATH = Path("config/cog-trust-bootstrap.v1.json")
PACKAGE_ROOT_RELATIVE_PATH = Path("trust-admissions")

EXPECTED_OWNER_ID = "256911919"
EXPECTED_REPOSITORY_ID = "1211713542"
EXPECTED_REPOSITORY_VISIBILITY = "public"
EXPECTED_REF = "refs/heads/main"
EXPECTED_EVENT = "workflow_dispatch"
EXPECTED_ADMISSION_WORKFLOW_REF = (
    "cognitum-one/cogs/.github/workflows/" "admit-cog-trust-staging.yml@refs/heads/main"
)
EXPECTED_ADMISSION_PROVIDER = (
    "projects/186366152200/locations/global/workloadIdentityPools/"
    "github-cog-authority-stg/providers/cogs-trust-registry-appender-stg"
)
EXPECTED_ADMISSION_SERVICE_ACCOUNT = (
    "cog-trust-append-stg@cognitum-20260110.iam.gserviceaccount.com"
)
EXPECTED_TRUST_BUCKET = "cognitum-20260110-cog-trust-stg"

CHANGE_ID = re.compile(r"^[a-z0-9][a-z0-9.-]{2,63}$")
COMMIT = re.compile(r"^[a-f0-9]{40}$")

AUTHORITY_TOPOLOGY: dict[str, dict[str, Any]] = {
    "release": {
        "kmsKeyVersion": (
            "projects/cognitum-20260110/locations/us-central1/keyRings/"
            "cog-release-stg/cryptoKeys/release-ed25519/cryptoKeyVersions/1"
        ),
        "builderIdentities": ["github-actions://cognitum-one/cogs"],
        "buildWorkflows": [
            "cognitum-one/cogs/.github/workflows/"
            "publish-cog-staging.yml@refs/heads/main"
        ],
        "workflowId": "322710413",
    },
    "withdrawal": {
        "kmsKeyVersion": (
            "projects/cognitum-20260110/locations/us-central1/keyRings/"
            "cog-withdrawal-stg/cryptoKeys/withdrawal-ed25519/"
            "cryptoKeyVersions/1"
        ),
        "builderIdentities": ["github-actions://cognitum-one/cogs:withdrawal"],
        "buildWorkflows": [
            "cognitum-one/cogs/.github/workflows/"
            "withdraw-cog-staging.yml@refs/heads/main"
        ],
        "workflowId": "323409651",
    },
}


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return f"sha256:{digest.hexdigest()}"


def _require_deterministic_json_file(
    path: Path,
    value: dict[str, Any],
    *,
    label: str,
) -> None:
    expected = (
        json.dumps(
            value,
            sort_keys=True,
            indent=2,
            ensure_ascii=False,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")
    if path.read_bytes() != expected:
        raise ReleaseError(f"{label} bytes are not deterministic JSON")


def _require_source_file(
    source_root: Path,
    relative_path: Path,
    *,
    label: str,
) -> Path:
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise ReleaseError(f"{label} path escapes the source checkout")
    resolved_root = source_root.resolve(strict=True)
    candidate = resolved_root / relative_path
    current = resolved_root
    for part in relative_path.parts:
        current = current / part
        if current.is_symlink():
            raise ReleaseError(f"{label} path contains a symlink")
    if not candidate.is_file():
        raise ReleaseError(f"{label} is not one tracked regular file")
    return candidate


def _require_exact_package(
    source_root: Path,
    change_id: str,
    *,
    has_predecessor: bool,
) -> tuple[Path, Path | None]:
    require_match(change_id, CHANGE_ID, "trust admission change id")
    package_relative = PACKAGE_ROOT_RELATIVE_PATH / change_id
    package = _require_source_file(
        source_root,
        package_relative / "registry.json",
        label="signed trust registry",
    ).parent
    expected = {"registry.json"}
    if has_predecessor:
        expected.add("previous-registry.json")
    actual = {entry.name for entry in package.iterdir()}
    if actual != expected:
        raise ReleaseError(
            "trust admission package must contain only the exact registry set"
        )
    if any(entry.is_symlink() or not entry.is_file() for entry in package.iterdir()):
        raise ReleaseError("trust admission package contains a non-regular file")
    previous = package / "previous-registry.json" if has_predecessor else None
    return package / "registry.json", previous


def _require_run_identity(args: argparse.Namespace) -> None:
    expected = {
        "GitHub owner id": (args.github_owner_id, EXPECTED_OWNER_ID),
        "GitHub repository id": (
            args.github_repository_id,
            EXPECTED_REPOSITORY_ID,
        ),
        "repository visibility": (
            args.repository_visibility,
            EXPECTED_REPOSITORY_VISIBILITY,
        ),
        "admission workflow ref": (
            args.admission_workflow_ref,
            EXPECTED_ADMISSION_WORKFLOW_REF,
        ),
        "source ref": (args.source_ref, EXPECTED_REF),
        "event": (args.event_name, EXPECTED_EVENT),
        "admission provider": (
            args.admission_wif_provider,
            EXPECTED_ADMISSION_PROVIDER,
        ),
        "admission service account": (
            args.admission_service_account,
            EXPECTED_ADMISSION_SERVICE_ACCOUNT,
        ),
        "trust bucket": (args.trust_bucket, EXPECTED_TRUST_BUCKET),
    }
    for label, (actual, required) in expected.items():
        if actual != required:
            raise ReleaseError(f"{label} is not the exact admission pin")
    for value, label in (
        (args.admission_workflow_id, "admission workflow id"),
        (args.actor_id, "admission actor id"),
    ):
        require_match(value, NUMERIC_GITHUB_ID, label)
    workflow_sha = require_match(
        args.admission_workflow_sha,
        COMMIT,
        "admission workflow SHA",
    )
    source_sha = require_match(args.source_sha, COMMIT, "admission source SHA")
    approved_sha = require_match(
        args.approved_admission_workflow_sha,
        COMMIT,
        "separately approved admission workflow SHA",
    )
    if workflow_sha != source_sha or workflow_sha != approved_sha:
        raise ReleaseError(
            "admission workflow SHA is not the separately approved source SHA"
        )


def _require_single_append(
    registry: dict[str, Any],
    previous: dict[str, Any] | None,
    *,
    purpose: str,
) -> dict[str, Any]:
    field = "releases" if purpose == "release" else "withdrawals"
    other = "withdrawals" if field == "releases" else "releases"
    if previous is None:
        if registry[other] != [] or len(registry[field]) != 1:
            raise ReleaseError(
                "genesis trust admission must append exactly one authority"
            )
        return registry[field][0]
    if registry[other] != previous[other]:
        raise ReleaseError("trust admission altered the other authority purpose")
    old_entries = previous[field]
    new_entries = registry[field]
    if len(new_entries) != len(old_entries) + 1 or new_entries[:-1] != old_entries:
        raise ReleaseError(
            "trust admission must append one authority without rewriting history"
        )
    return new_entries[-1]


def _require_new_authority(
    entry: dict[str, Any],
    args: argparse.Namespace,
) -> None:
    topology = AUTHORITY_TOPOLOGY[args.purpose]
    expected_workflow_id = require_match(
        args.admitted_workflow_id,
        NUMERIC_GITHUB_ID,
        "admitted workflow id",
    )
    if expected_workflow_id != topology["workflowId"]:
        raise ReleaseError("admitted numeric workflow id is not approved")
    admitted_sha = require_match(
        args.admitted_workflow_sha,
        COMMIT,
        "admitted workflow SHA",
    )
    expected = {
        "keyId": require_match(args.key_id, KEY_ID, "admitted key id"),
        "purpose": args.purpose,
        "kmsKeyVersion": topology["kmsKeyVersion"],
        "publicKeyFingerprint": require_digest(
            args.public_key_fingerprint,
            "admitted public-key fingerprint",
        ),
        "builderIdentities": topology["builderIdentities"],
        "buildWorkflows": topology["buildWorkflows"],
        "workflowSha": admitted_sha,
        "github": {
            "ownerId": EXPECTED_OWNER_ID,
            "repositoryId": EXPECTED_REPOSITORY_ID,
            "workflowIds": [expected_workflow_id],
        },
        "status": "active",
        "revocation": None,
    }
    for field, value in expected.items():
        if entry.get(field) != value:
            raise ReleaseError(
                f"new {args.purpose} authority differs from {field} admission pin"
            )


def verify_admission(
    args: argparse.Namespace,
    *,
    source_root: Path = SOURCE_ROOT,
) -> dict[str, Any]:
    _require_run_identity(args)
    if args.purpose not in AUTHORITY_TOPOLOGY:
        raise ReleaseError("unsupported trust admission purpose")
    workflow_approval_digest = require_digest(
        args.workflow_sha_approval_digest,
        "workflow-SHA approval receipt digest",
    )
    expected_bootstrap_digest = require_digest(
        args.expected_bootstrap_digest,
        "expected trust bootstrap digest",
    )
    expected_registry_digest = require_digest(
        args.expected_registry_digest,
        "expected trust registry digest",
    )
    if (
        not isinstance(args.expected_head_sequence, int)
        or isinstance(args.expected_head_sequence, bool)
        or not 0 <= args.expected_head_sequence < 2**53 - 1
    ):
        raise ReleaseError("expected trust-registry head sequence is invalid")
    if args.expected_head_sequence == 0:
        if args.expected_head_digest != GENESIS:
            raise ReleaseError("pre-genesis trust head must be GENESIS")
    else:
        require_digest(
            args.expected_head_digest,
            "expected trust-registry head digest",
        )

    bootstrap_path = _require_source_file(
        source_root,
        BOOTSTRAP_RELATIVE_PATH,
        label="source trust bootstrap",
    )
    bootstrap = validate_bootstrap(read_json(bootstrap_path))
    if bootstrap_digest(bootstrap) != expected_bootstrap_digest:
        raise ReleaseError("source bootstrap differs from the protected digest pin")

    registry_path, previous_path = _require_exact_package(
        source_root,
        args.change_id,
        has_predecessor=args.expected_head_sequence > 0,
    )
    registry = validate_registry(read_json(registry_path), signed=True)
    _require_deterministic_json_file(
        registry_path,
        registry,
        label="signed trust registry",
    )
    if registry["sequence"] != args.expected_head_sequence + 1:
        raise ReleaseError("trust admission is not the single next sequence")
    if registry["previousRegistryDigest"] != args.expected_head_digest:
        raise ReleaseError("trust admission does not extend the protected head")
    if registry_payload_digest(registry) != expected_registry_digest:
        raise ReleaseError("trust admission registry digest differs from review")

    previous: dict[str, Any] | None = None
    if previous_path is not None:
        previous = validate_registry(read_json(previous_path), signed=True)
        _require_deterministic_json_file(
            previous_path,
            previous,
            label="signed trust-registry predecessor",
        )
        if previous["sequence"] != args.expected_head_sequence:
            raise ReleaseError("packaged predecessor sequence differs from the head")
        if registry_payload_digest(previous) != args.expected_head_digest:
            raise ReleaseError("packaged predecessor digest differs from the head")

    verify_trust_registry(
        argparse.Namespace(
            bootstrap=bootstrap_path,
            registry=registry_path,
            expected_bootstrap_digest=expected_bootstrap_digest,
            expected_registry_digest=expected_registry_digest,
            minimum_sequence=registry["sequence"],
            previous_registry=previous_path,
            checked_at=args.checked_at,
        )
    )
    entry = _require_single_append(
        registry,
        previous,
        purpose=args.purpose,
    )
    _require_new_authority(entry, args)

    previous_token = (
        GENESIS
        if args.expected_head_digest == GENESIS
        else args.expected_head_digest.removeprefix("sha256:")
    )
    destination = (
        f"gs://{EXPECTED_TRUST_BUCKET}/staging/cogs/trust/registries/v3/"
        f"sequence-{registry['sequence']:020d}/from-{previous_token}/"
        "release-trust-registry.json"
    )
    plan = {
        "schema": "cognitum.cog.trust-admission-plan.v1",
        "status": "VERIFIED_SOURCE_ONLY",
        "deploymentAuthority": False,
        "changeId": args.change_id,
        "checkedAt": args.checked_at,
        "sourceSha": args.source_sha,
        "admissionWorkflow": {
            "id": args.admission_workflow_id,
            "ref": args.admission_workflow_ref,
            "sha": args.admission_workflow_sha,
            "actorId": args.actor_id,
        },
        "bootstrap": {
            "path": BOOTSTRAP_RELATIVE_PATH.as_posix(),
            "digest": expected_bootstrap_digest,
        },
        "previousRegistry": {
            "sequence": args.expected_head_sequence,
            "digest": args.expected_head_digest,
        },
        "registry": {
            "path": registry_path.relative_to(source_root.resolve()).as_posix(),
            "fileDigest": _sha256_file(registry_path),
            "payloadDigest": expected_registry_digest,
            "sequence": registry["sequence"],
            "destination": destination,
        },
        "newAuthority": {
            "purpose": args.purpose,
            "keyId": args.key_id,
            "kmsKeyVersion": entry["kmsKeyVersion"],
            "publicKeyFingerprint": args.public_key_fingerprint,
            "workflowId": args.admitted_workflow_id,
            "workflowSha": args.admitted_workflow_sha,
        },
        "workflowShaApprovalReceiptDigest": workflow_approval_digest,
        "postAppendActions": {
            "rotateFederationProvider": False,
            "updateRuntimePin": False,
            "seedProjection": False,
            "retryOnAmbiguousCreate": False,
            "requireIndependentReadbackAndReceiptAttestation": True,
        },
    }
    write_json(args.output, plan)
    return plan


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description="Verify one already-quorum-signed trust-registry append"
    )
    result.add_argument("--change-id", required=True)
    result.add_argument("--expected-bootstrap-digest", required=True)
    result.add_argument("--expected-registry-digest", required=True)
    result.add_argument("--expected-head-sequence", required=True, type=int)
    result.add_argument("--expected-head-digest", required=True)
    result.add_argument(
        "--purpose",
        required=True,
        choices=tuple(AUTHORITY_TOPOLOGY),
    )
    result.add_argument("--key-id", required=True)
    result.add_argument("--public-key-fingerprint", required=True)
    result.add_argument("--admitted-workflow-id", required=True)
    result.add_argument("--admitted-workflow-sha", required=True)
    result.add_argument("--workflow-sha-approval-digest", required=True)
    result.add_argument("--admission-wif-provider", required=True)
    result.add_argument("--admission-service-account", required=True)
    result.add_argument("--trust-bucket", required=True)
    result.add_argument("--github-owner-id", required=True)
    result.add_argument("--github-repository-id", required=True)
    result.add_argument("--repository-visibility", required=True)
    result.add_argument("--admission-workflow-id", required=True)
    result.add_argument("--admission-workflow-ref", required=True)
    result.add_argument("--admission-workflow-sha", required=True)
    result.add_argument("--approved-admission-workflow-sha", required=True)
    result.add_argument("--source-sha", required=True)
    result.add_argument("--source-ref", required=True)
    result.add_argument("--event-name", required=True)
    result.add_argument("--actor-id", required=True)
    result.add_argument("--checked-at", required=True)
    result.add_argument("--output", required=True, type=Path)
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        plan = verify_admission(args)
    except (OSError, ValueError, json.JSONDecodeError, ReleaseError) as error:
        print(f"trust admission rejected: {error}")
        return 1
    print(json.dumps(plan, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
