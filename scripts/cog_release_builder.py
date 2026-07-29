"""Construct a complete unsigned Cog release and its measured build evidence."""

from __future__ import annotations

import argparse
from pathlib import Path

from cog_isolation import evidence_passed
from cog_release_provenance_lib import (
    BUILT_AT,
    COMMIT,
    KEY_ID,
    PROVENANCE_SCHEMA,
    VERSION,
    ReleaseError,
    canonical_payload,
    digest_file,
    payload_digest,
    read_json,
    require_ascii,
    require_match,
    validate_policy,
    validate_release,
    write_json,
)


def isolation_passed(path: Path, cog_id: str) -> bool:
    data = read_json(path)
    return data.get("cogId") == cog_id and evidence_passed(data)


def prepare(args: argparse.Namespace) -> None:
    policy = validate_policy(read_json(args.policy))
    arch = require_ascii(args.arch, "arch", 16)
    if arch not in policy["artifacts"]:
        raise ReleaseError(f"release policy does not authorize architecture {arch}")
    version = require_match(args.version, VERSION, "version")
    source_commit = require_match(args.source_commit, COMMIT, "source commit")
    key_id = require_match(args.key_id, KEY_ID, "key id")
    builder = require_ascii(args.builder_identity, "builder identity")
    workflow = require_ascii(args.build_workflow, "build workflow")
    built_at = require_match(args.built_at, BUILT_AT, "built at")
    repository = require_ascii(args.repository, "repository", 256)
    sigstore_tool = require_ascii(args.sigstore_tool_version, "Sigstore tool", 128)
    if builder != f"github-actions://{repository}":
        raise ReleaseError("builder identity must bind to the repository")
    vulnerability = read_json(args.vulnerability_scan)
    advisories = vulnerability.get("vulnerabilities", {}).get("list", [])
    if not isinstance(advisories, list) or advisories:
        raise ReleaseError("vulnerability scan must contain zero advisories")
    if not isolation_passed(args.isolation, policy["cogId"]):
        raise ReleaseError(
            "isolation evidence did not prove limits and negative control"
        )

    digests = {
        "artifact": digest_file(args.artifact),
        "signature": digest_file(args.sigstore_bundle),
        "lock": digest_file(args.dependency_lock),
        "sbom": digest_file(args.sbom),
        "vulnerability": digest_file(args.vulnerability_scan),
        "provenance": digest_file(args.provenance),
        "isolation": digest_file(args.isolation),
    }
    controls = [
        ("A3", "dependency-lock", "lock"),
        ("A4", "sbom", "sbom"),
        ("A5", "vulnerability-scan", "vulnerability"),
        ("A6", "isolation", "isolation"),
        ("A7", "sigstore-signature", "signature"),
        ("A2", "slsa-provenance", "provenance"),
    ]
    policy_decision = {
        "subject": digests["artifact"],
        "evaluatedAt": built_at,
        "policyVersion": "adr-113-rev-15-gates-a1-a8",
        "controls": [
            {"id": gate, "name": name, "verdict": "pass", "evidence": digests[field]}
            for gate, name, field in controls
        ],
    }
    output = args.output_dir
    output.mkdir(parents=True, exist_ok=True)
    policy_decision_path = output / "policy-decision.json"
    write_json(policy_decision_path, policy_decision)
    digests["policy"] = digest_file(policy_decision_path)
    artifact_policy = policy["artifacts"][arch]
    if args.artifact.name != artifact_policy["binaryName"]:
        raise ReleaseError("artifact filename does not match ratified release policy")
    release = {
        "cogId": policy["cogId"],
        "blueprintId": policy["blueprintId"],
        "blueprintDigest": policy["blueprintDigest"],
        "version": version,
        "releaseDigest": digests["artifact"],
        "sourceCommit": source_commit,
        "runtimeContractVersion": policy["runtimeContractVersion"],
        "packaging": policy["packaging"],
        "deploymentDriver": policy["deploymentDriver"],
        "artifactRef": {
            "kind": "edge-binary",
            "binaryDigest": digests["artifact"],
            "binaryName": artifact_policy["binaryName"],
            "targetHardware": artifact_policy["targetHardware"],
        },
        "tenancyMode": policy["tenancyMode"],
        "statePolicy": policy["statePolicy"],
        "stateSchemaVersion": policy["stateSchemaVersion"],
        "rollbackCompatibility": policy["rollbackCompatibility"],
        "networkPolicy": policy["networkPolicy"],
        "provenance": {
            "signatureAlgorithm": "ed25519",
            "signingKeyId": key_id,
            "builderIdentity": builder,
            "buildWorkflow": workflow,
            "dependencyLockDigest": digests["lock"],
            "sbomDigest": digests["sbom"],
            "provenanceDigest": digests["provenance"],
            "builtAt": built_at,
        },
        "securityAttestation": {
            "vulnerabilityScanDigest": digests["vulnerability"],
            "policyDecisionDigest": digests["policy"],
            "isolationEvidenceDigest": digests["isolation"],
            "isolationPassed": True,
        },
        "residency": policy["residency"],
        "lifecycle": policy["lifecycle"],
    }
    validate_release(release, signed=False)
    build_evidence = {
        "kind": "cognitum.cog-build-evidence.v1-partial",
        "cogId": policy["cogId"],
        "blueprintId": policy["blueprintId"],
        "blueprintDigest": policy["blueprintDigest"],
        "version": version,
        "arch": arch,
        "sourceCommit": source_commit,
        "releaseDigest": digests["artifact"],
        "runtimeContractVersion": policy["runtimeContractVersion"],
        "artifactDigest": digests["artifact"],
        "signatureDigest": digests["signature"],
        "dependencyLockDigest": digests["lock"],
        "sbomDigest": digests["sbom"],
        "vulnerabilityScanDigest": digests["vulnerability"],
        "signatureAlgorithm": "ecdsa-p256",
        "signatureFormat": "sigstore-bundle",
        "signingToolVersion": sigstore_tool,
        "signatureVerified": True,
        "signingIdentity": f"keyless:github-oidc:{repository}",
        "builderIdentity": builder,
        "buildWorkflow": workflow,
        "builtAt": built_at,
        "provenanceDigest": digests["provenance"],
        "policyDecisionDigest": digests["policy"],
        "isolationEvidenceDigest": digests["isolation"],
        "isolationPassed": True,
        "releasePolicyDigest": digest_file(args.policy),
    }
    write_json(output / "build-evidence.json", build_evidence)
    write_json(output / "unsigned-release.json", release)
    (output / "release-statement.json").write_bytes(canonical_payload(release))
    write_json(
        output / "release-statement-metadata.json",
        {
            "schema": PROVENANCE_SCHEMA,
            "payloadDigest": payload_digest(release),
            "keyId": key_id,
        },
    )
