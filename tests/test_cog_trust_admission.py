from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from cog_release_provenance_lib import ReleaseError  # noqa: E402
from cog_trust_admission import (  # noqa: E402
    AUTHORITY_TOPOLOGY,
    EXPECTED_ADMISSION_PROVIDER,
    EXPECTED_ADMISSION_SERVICE_ACCOUNT,
    EXPECTED_ADMISSION_WORKFLOW_REF,
    EXPECTED_EVENT,
    EXPECTED_OWNER_ID,
    EXPECTED_REF,
    EXPECTED_REPOSITORY_ID,
    EXPECTED_REPOSITORY_VISIBILITY,
    EXPECTED_TRUST_BUCKET,
    verify_admission,
)
from cog_trust_registry import (  # noqa: E402
    ROOT_IDENTITIES,
    bootstrap_digest,
    canonical_registry_payload,
    registry_payload_digest,
    validate_bootstrap,
)

WORKFLOW = ROOT / ".github/workflows/admit-cog-trust-staging.yml"


def run(*command: str) -> bytes:
    result = subprocess.run(command, check=True, capture_output=True)
    return result.stdout


class CogTrustAdmissionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.source_root = Path(self.temporary.name)
        (self.source_root / "config").mkdir()
        self.package = self.source_root / "trust-admissions/trust-2026-07"
        self.package.mkdir(parents=True)
        self.keys = self.source_root / "keys"
        self.keys.mkdir()

        roots = []
        self.root_signers: list[tuple[str, Path]] = []
        for index, (role, key_id) in enumerate(ROOT_IDENTITIES.items(), start=1):
            private, public, fingerprint = self._generate_key(f"root-{index}")
            roots.append(
                {
                    "role": role,
                    "keyId": key_id,
                    "signingResource": (
                        "projects/cognitum-20260110/locations/us-central1/"
                        f"keyRings/cog-trust-root-{index}/cryptoKeys/"
                        "root-ed25519/cryptoKeyVersions/1"
                    ),
                    "algorithm": "ed25519",
                    "publicKeyFingerprint": fingerprint,
                    "publicKeyPem": public.read_text(),
                }
            )
            self.root_signers.append((key_id, private))
        self.bootstrap = {
            "schema": "cognitum.cog.trust-bootstrap.v1",
            "threshold": 2,
            "roots": roots,
        }
        (self.source_root / "config/cog-trust-bootstrap.v1.json").write_text(
            json.dumps(self.bootstrap)
        )

        _, authority_public, authority_fingerprint = self._generate_key(
            "release-authority"
        )
        self.authority_fingerprint = authority_fingerprint
        self.release_entry = self._entry(
            "release",
            public_key=authority_public,
            fingerprint=authority_fingerprint,
            workflow_sha="a" * 40,
        )
        unsigned = {
            "schema": "cognitum.cog.trust-registry.v3",
            "sequence": 1,
            "previousRegistryDigest": "GENESIS",
            "issuedAt": "2026-07-30T12:00:00Z",
            "notBefore": "2026-07-30T12:00:00Z",
            "expiresAt": "2026-10-27T12:00:00Z",
            "releases": [self.release_entry],
            "withdrawals": [],
        }
        self.registry = self._sign_registry(unsigned)
        self._write_registry(self.registry)
        self.args = self._args()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _generate_key(self, name: str) -> tuple[Path, Path, str]:
        private = self.keys / f"{name}.private.pem"
        public = self.keys / f"{name}.public.pem"
        run(
            "openssl",
            "genpkey",
            "-algorithm",
            "ED25519",
            "-out",
            str(private),
        )
        run(
            "openssl",
            "pkey",
            "-in",
            str(private),
            "-pubout",
            "-out",
            str(public),
        )
        der = run(
            "openssl",
            "pkey",
            "-pubin",
            "-in",
            str(public),
            "-outform",
            "DER",
        )
        return private, public, f"sha256:{hashlib.sha256(der).hexdigest()}"

    def _entry(
        self,
        purpose: str,
        *,
        public_key: Path,
        fingerprint: str,
        workflow_sha: str,
    ) -> dict:
        topology = AUTHORITY_TOPOLOGY[purpose]
        return {
            "keyId": f"cog-{purpose}-staging-2026-07",
            "algorithm": "ed25519",
            "kmsAlgorithm": "EC_SIGN_ED25519",
            "kmsKeyVersion": topology["kmsKeyVersion"],
            "protectionLevel": "software",
            "publicKeyFingerprint": fingerprint,
            "publicKeyPem": public_key.read_text(),
            "status": "active",
            "purpose": purpose,
            "notBefore": "2026-07-30T12:00:00Z",
            "expiresAt": "2027-07-29T12:00:00Z",
            "builderIdentities": topology["builderIdentities"],
            "buildWorkflows": topology["buildWorkflows"],
            "workflowSha": workflow_sha,
            "github": {
                "ownerId": EXPECTED_OWNER_ID,
                "repositoryId": EXPECTED_REPOSITORY_ID,
                "workflowIds": [topology["workflowId"]],
            },
            "revocation": None,
        }

    def _sign_registry(
        self,
        value: dict,
        *,
        signers: list[tuple[str, Path]] | None = None,
    ) -> dict:
        unsigned = copy.deepcopy(value)
        unsigned.pop("signatures", None)
        payload = self.keys / "registry-statement.json"
        payload.write_bytes(canonical_registry_payload(unsigned))
        signatures = []
        for index, (key_id, private) in enumerate(signers or self.root_signers[:2]):
            signature = self.keys / f"registry-{index}.sig"
            run(
                "openssl",
                "pkeyutl",
                "-sign",
                "-inkey",
                str(private),
                "-rawin",
                "-in",
                str(payload),
                "-out",
                str(signature),
            )
            signatures.append(
                {
                    "schema": "cognitum.cog.trust-registry.v3",
                    "algorithm": "ed25519",
                    "keyId": key_id,
                    "payloadDigest": registry_payload_digest(unsigned),
                    "signature": base64.urlsafe_b64encode(signature.read_bytes())
                    .decode("ascii")
                    .rstrip("="),
                }
            )
        unsigned["signatures"] = signatures
        return unsigned

    def _write_registry(self, value: dict) -> None:
        (self.package / "registry.json").write_text(
            json.dumps(value, sort_keys=True, indent=2) + "\n"
        )

    def _args(self) -> argparse.Namespace:
        return argparse.Namespace(
            change_id="trust-2026-07",
            expected_bootstrap_digest=bootstrap_digest(self.bootstrap),
            expected_registry_digest=registry_payload_digest(self.registry),
            expected_head_sequence=0,
            expected_head_digest="GENESIS",
            purpose="release",
            key_id=self.release_entry["keyId"],
            public_key_fingerprint=self.authority_fingerprint,
            admitted_workflow_id=AUTHORITY_TOPOLOGY["release"]["workflowId"],
            admitted_workflow_sha="a" * 40,
            workflow_sha_approval_digest="sha256:" + "c" * 64,
            admission_wif_provider=EXPECTED_ADMISSION_PROVIDER,
            admission_service_account=EXPECTED_ADMISSION_SERVICE_ACCOUNT,
            trust_bucket=EXPECTED_TRUST_BUCKET,
            github_owner_id=EXPECTED_OWNER_ID,
            github_repository_id=EXPECTED_REPOSITORY_ID,
            repository_visibility=EXPECTED_REPOSITORY_VISIBILITY,
            admission_workflow_id="333333333",
            admission_workflow_ref=EXPECTED_ADMISSION_WORKFLOW_REF,
            admission_workflow_sha="b" * 40,
            approved_admission_workflow_sha="b" * 40,
            source_sha="b" * 40,
            source_ref=EXPECTED_REF,
            event_name=EXPECTED_EVENT,
            actor_id="44444",
            checked_at="2026-07-30T12:01:00Z",
            output=self.source_root / "out/plan.json",
        )

    def _verify(self, args: argparse.Namespace | None = None) -> dict:
        return verify_admission(
            args or self.args,
            source_root=self.source_root,
        )

    def test_two_root_genesis_emits_source_only_single_append_plan(self) -> None:
        plan = self._verify()
        self.assertEqual(plan["status"], "VERIFIED_SOURCE_ONLY")
        self.assertFalse(plan["deploymentAuthority"])
        self.assertEqual(plan["previousRegistry"]["digest"], "GENESIS")
        self.assertEqual(plan["registry"]["sequence"], 1)
        self.assertIn(
            "/sequence-00000000000000000001/from-GENESIS/",
            plan["registry"]["destination"],
        )
        self.assertEqual(plan["newAuthority"]["purpose"], "release")
        self.assertEqual(
            plan["postAppendActions"],
            {
                "rotateFederationProvider": False,
                "updateRuntimePin": False,
                "seedProjection": False,
                "retryOnAmbiguousCreate": False,
                "requireIndependentReadbackAndReceiptAttestation": True,
            },
        )

    def test_quorum_source_and_topology_mutations_fail_closed(self) -> None:
        mutations: list[tuple[str, Callable[[dict, argparse.Namespace], object]]] = [
            (
                "one-root",
                lambda value, args: value.update(signatures=value["signatures"][:1]),
            ),
            (
                "duplicate-root",
                lambda value, args: value["signatures"][1].update(
                    keyId=value["signatures"][0]["keyId"]
                ),
            ),
            (
                "wrong-owner-id",
                lambda value, args: value["releases"][0]["github"].update(
                    ownerId="99999"
                ),
            ),
            (
                "wrong-repository-id",
                lambda value, args: value["releases"][0]["github"].update(
                    repositoryId="99999"
                ),
            ),
            (
                "wrong-workflow-id",
                lambda value, args: value["releases"][0]["github"].update(
                    workflowIds=["99999"]
                ),
            ),
            (
                "wrong-workflow-sha",
                lambda value, args: value["releases"][0].update(workflowSha="d" * 40),
            ),
            (
                "wrong-kms-purpose-boundary",
                lambda value, args: value["releases"][0].update(
                    kmsKeyVersion=AUTHORITY_TOPOLOGY["withdrawal"]["kmsKeyVersion"]
                ),
            ),
            (
                "wrong-builder",
                lambda value, args: value["releases"][0].update(
                    builderIdentities=["github-actions://attacker/fork"]
                ),
            ),
            (
                "wrong-build-workflow",
                lambda value, args: value["releases"][0].update(
                    buildWorkflows=AUTHORITY_TOPOLOGY["withdrawal"]["buildWorkflows"]
                ),
            ),
            (
                "cross-purpose",
                lambda value, args: (
                    value["withdrawals"].append(
                        {
                            **value["releases"].pop(),
                            "purpose": "withdrawal",
                        }
                    )
                ),
            ),
        ]
        signature_only = {"one-root", "duplicate-root"}
        for name, mutation in mutations:
            with self.subTest(name=name):
                value = copy.deepcopy(self.registry)
                args = copy.deepcopy(self.args)
                mutation(value, args)
                if name not in signature_only:
                    value = self._sign_registry(value)
                self._write_registry(value)
                args.expected_registry_digest = registry_payload_digest(value)
                with self.assertRaises(ReleaseError):
                    self._verify(args)
        self._write_registry(self.registry)

    def test_sequence_two_preserves_history_and_appends_one_other_purpose(
        self,
    ) -> None:
        (self.package / "previous-registry.json").write_text(
            json.dumps(self.registry, sort_keys=True, indent=2) + "\n"
        )
        _, public_key, fingerprint = self._generate_key("withdrawal-authority")
        withdrawal = self._entry(
            "withdrawal",
            public_key=public_key,
            fingerprint=fingerprint,
            workflow_sha="e" * 40,
        )
        unsigned = copy.deepcopy(self.registry)
        unsigned.pop("signatures")
        unsigned.update(
            {
                "sequence": 2,
                "previousRegistryDigest": registry_payload_digest(self.registry),
                "issuedAt": "2026-07-30T12:02:00Z",
                "notBefore": "2026-07-30T12:02:00Z",
                "expiresAt": "2026-10-27T12:02:00Z",
                "withdrawals": [withdrawal],
            }
        )
        candidate = self._sign_registry(unsigned)
        self._write_registry(candidate)
        args = copy.deepcopy(self.args)
        args.expected_head_sequence = 1
        args.expected_head_digest = registry_payload_digest(self.registry)
        args.expected_registry_digest = registry_payload_digest(candidate)
        args.purpose = "withdrawal"
        args.key_id = withdrawal["keyId"]
        args.public_key_fingerprint = fingerprint
        args.admitted_workflow_id = AUTHORITY_TOPOLOGY["withdrawal"]["workflowId"]
        args.admitted_workflow_sha = "e" * 40
        args.checked_at = "2026-07-30T12:03:00Z"
        plan = self._verify(args)
        self.assertEqual(plan["registry"]["sequence"], 2)
        self.assertEqual(plan["newAuthority"]["purpose"], "withdrawal")

        rewritten = copy.deepcopy(candidate)
        rewritten["releases"][0]["workflowSha"] = "f" * 40
        rewritten = self._sign_registry(rewritten)
        self._write_registry(rewritten)
        args.expected_registry_digest = registry_payload_digest(rewritten)
        with self.assertRaises(ReleaseError):
            self._verify(args)

    def test_run_head_and_review_pin_mutations_fail_closed(self) -> None:
        cases = {
            "bootstrap-digest": (
                "expected_bootstrap_digest",
                "sha256:" + "0" * 64,
            ),
            "registry-digest": (
                "expected_registry_digest",
                "sha256:" + "0" * 64,
            ),
            "head-gap": ("expected_head_sequence", 2),
            "head-substitution": (
                "expected_head_digest",
                "sha256:" + "0" * 64,
            ),
            "workflow-approval": (
                "workflow_sha_approval_digest",
                "not-a-digest",
            ),
            "provider": ("admission_wif_provider", "attacker-provider"),
            "service-account": (
                "admission_service_account",
                "cog-release-publisher-stg@cognitum-20260110.iam.gserviceaccount.com",
            ),
            "bucket": ("trust_bucket", "cognitum-20260110-cog-release-stg"),
            "run-owner": ("github_owner_id", "99999"),
            "run-repository": ("github_repository_id", "99999"),
            "run-ref": ("source_ref", "refs/pull/1/merge"),
            "run-event": ("event_name", "pull_request"),
            "run-workflow-ref": (
                "admission_workflow_ref",
                "cognitum-one/cogs/.github/workflows/"
                "publish-cog-staging.yml@refs/heads/main",
            ),
            "run-workflow-sha": ("admission_workflow_sha", "d" * 40),
            "approved-run-sha": (
                "approved_admission_workflow_sha",
                "d" * 40,
            ),
            "source-sha": ("source_sha", "d" * 40),
            "actor-not-numeric": ("actor_id", "attacker"),
            "admitted-workflow-id": ("admitted_workflow_id", "99999"),
            "admitted-workflow-sha": ("admitted_workflow_sha", "d" * 40),
            "fingerprint-review": (
                "public_key_fingerprint",
                "sha256:" + "0" * 64,
            ),
        }
        for name, (field, value) in cases.items():
            with self.subTest(name=name):
                args = copy.deepcopy(self.args)
                setattr(args, field, value)
                with self.assertRaises(ReleaseError):
                    self._verify(args)

    def test_package_is_exact_and_cannot_use_symlink_or_extra_file(self) -> None:
        extra = self.package / "unreviewed.json"
        extra.write_text("{}")
        with self.assertRaises(ReleaseError):
            self._verify()
        extra.unlink()

        (self.package / "registry.json").write_text(json.dumps(self.registry))
        with self.assertRaises(ReleaseError):
            self._verify()
        self._write_registry(self.registry)

        registry = self.package / "registry.json"
        target = self.package / "target.json"
        registry.rename(target)
        registry.symlink_to(target.name)
        with self.assertRaises(ReleaseError):
            self._verify()


class CogTrustAdmissionWorkflowTests(unittest.TestCase):
    def test_proposed_source_bootstrap_has_exact_self_consistent_public_roots(
        self,
    ) -> None:
        path = ROOT / "config/cog-trust-bootstrap.v1.json"
        bootstrap = validate_bootstrap(json.loads(path.read_text()))
        self.assertEqual(bootstrap["threshold"], 2)
        self.assertEqual(
            {root["role"] for root in bootstrap["roots"]},
            set(ROOT_IDENTITIES),
        )
        with tempfile.TemporaryDirectory() as directory:
            for index, root in enumerate(bootstrap["roots"]):
                public_key = Path(directory) / f"root-{index}.pem"
                public_key.write_text(root["publicKeyPem"])
                der = run(
                    "openssl",
                    "pkey",
                    "-pubin",
                    "-in",
                    str(public_key),
                    "-outform",
                    "DER",
                )
                self.assertEqual(
                    root["publicKeyFingerprint"],
                    f"sha256:{hashlib.sha256(der).hexdigest()}",
                )

    def test_workflow_is_pinned_create_only_and_source_only(self) -> None:
        source = WORKFLOW.read_text()
        for token in (
            "environment: cogs-trust-admission-staging",
            "scripts/cog_trust_admission.py",
            "GCP_COGS_TRUST_STAGING_APPROVED_WORKFLOW_SHA",
            "GCP_COGS_TRUST_STAGING_APPROVED_REGISTRY_DIGEST",
            "GCP_COGS_TRUST_STAGING_REGISTRY_HEAD_SEQUENCE",
            "GCP_COGS_TRUST_STAGING_REGISTRY_HEAD_DIGEST",
            "--if-generation-match=0",
            "--print-created-message",
            '"status": "UNKNOWN"',
            '"retryAllowed": False',
            "never retry, overwrite, or use an alternate path",
            '"receiptAttestation": "PENDING_EXTERNAL"',
            '"deploymentAuthority": False',
            "No provider, runtime pin, Firestore projection, or production state",
        ):
            self.assertIn(token, source)
        for forbidden in (
            "kms asymmetric-sign",
            "gcloud storage ls",
            "gcloud storage cat",
            "gcloud storage rm",
            "gcloud storage mv",
            "createDocument",
            "datastore.entities",
            "workload-identity-pools providers update",
            "--if-generation-match=1",
            "credentials_json",
        ):
            self.assertNotIn(forbidden, source)
        for line in source.splitlines():
            if "uses:" in line:
                revision = line.rsplit("@", 1)[-1].strip()
                self.assertRegex(revision, r"^[a-f0-9]{40}$")
            if "actions/checkout@" in line:
                self.assertIn("persist-credentials: false", source)

    def test_all_authority_workflows_use_canonical_bootstrap_and_identities(
        self,
    ) -> None:
        withdrawal = (ROOT / ".github/workflows/withdraw-cog-staging.yml").read_text()
        release = (ROOT / ".github/workflows/publish-cog-staging.yml").read_text()
        self.assertIn("config/cog-trust-bootstrap.v1.json", withdrawal)
        self.assertNotIn("config/cog-trust-bootstrap.json", withdrawal)
        self.assertIn(
            '--builder-identity "github-actions://cognitum-one/cogs"',
            release,
        )
        self.assertIn(
            '--issuer-identity "github-actions://cognitum-one/cogs:withdrawal"',
            withdrawal,
        )


if __name__ == "__main__":
    unittest.main()
