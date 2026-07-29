"""Security and determinism tests for optional Cog runtime integrations."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from cog_integrations import (  # noqa: E402
    ManifestValidationError,
    canonical_bytes,
    emit_manifest,
    load_manifest,
    normalize_manifest,
)
from cog_website_bundle import (  # noqa: E402
    WebsiteBundleError,
    inspect_profile,
    package_website,
)

FIXTURES = ROOT / "tests" / "fixtures" / "cog-integrations"


def fixture_data(name: str) -> dict:
    with (FIXTURES / name).open("rb") as stream:
        return tomllib.load(stream)


class CogIntegrationManifestTests(unittest.TestCase):
    def test_absence_normalizes_all_interfaces_off(self) -> None:
        result = load_manifest(FIXTURES / "valid" / "default-disabled.toml")
        self.assertEqual(
            result["integrations"],
            {
                "website": {"enabled": False},
                "tailscale": {"enabled": False},
                "webMcp": {"enabled": False},
            },
        )

    def test_all_enabled_maps_to_platform_field_names_and_defaults(self) -> None:
        result = load_manifest(FIXTURES / "valid" / "all-enabled.toml")
        website = result["integrations"]["website"]
        tailscale = result["integrations"]["tailscale"]
        mcp = result["integrations"]["webMcp"]
        self.assertEqual(website["artifact"]["buildProfile"], "vite-production-v1")
        self.assertNotIn("buildCommand", website["artifact"])
        self.assertEqual(tailscale["credentialBinding"], "tailscale-oauth-client")
        self.assertNotIn("secretManagerRef", json.dumps(tailscale))
        self.assertEqual(mcp["transport"], "streamable-http")
        self.assertEqual(mcp["protocolVersion"], "2025-11-25")
        self.assertEqual(
            mcp["auth"]["resourceMetadataUrl"].split("/")[2], "mcp.example.com"
        )

    def test_emission_is_canonical_and_content_addressed(self) -> None:
        source = FIXTURES / "valid" / "all-enabled.toml"
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            first_path, first_digest, first_sum = emit_manifest(source, Path(first))
            second_path, second_digest, second_sum = emit_manifest(source, Path(second))
            self.assertEqual(first_digest, second_digest)
            self.assertEqual(first_path.name, second_path.name)
            self.assertEqual(first_path.read_bytes(), second_path.read_bytes())
            self.assertEqual(
                hashlib.sha256(first_path.read_bytes()).hexdigest(), first_digest
            )
            self.assertIn(first_digest, first_path.name)
            self.assertEqual(first_sum.read_text(), second_sum.read_text())
            parsed = json.loads(first_path.read_text())
            self.assertEqual(first_path.read_bytes(), canonical_bytes(parsed))

    def test_invalid_security_fixtures_fail_with_expected_code(self) -> None:
        cases = {
            "invalid/arbitrary-build-command.toml": "unknown-field",
            "invalid/tailscale-secret-reference.toml": "unknown-field",
            "invalid/public-mcp-without-oauth-or-approval.toml": "oauth-required",
            "invalid/privileged-mcp-tool.toml": "forbidden-authority",
        }
        for relative, expected in cases.items():
            with self.subTest(relative=relative):
                with self.assertRaises(ManifestValidationError) as raised:
                    load_manifest(FIXTURES / relative)
                self.assertIn(
                    expected, {issue.code for issue in raised.exception.issues}
                )

    def test_oci_tags_and_mutable_references_are_rejected(self) -> None:
        data = fixture_data("valid/all-enabled.toml")
        artifact = data["integrations"]["website"]["artifact"]
        artifact.clear()
        artifact.update(
            {"kind": "oci-image", "image": "us-docker.pkg.dev/example/cog:latest"}
        )
        with self.assertRaises(ManifestValidationError) as raised:
            normalize_manifest(data)
        self.assertIn(
            "immutable-image-required",
            {issue.code for issue in raised.exception.issues},
        )

    def test_legacy_sse_requires_explicit_acknowledgement(self) -> None:
        data = fixture_data("valid/all-enabled.toml")
        data["integrations"]["web_mcp"]["transport"] = "legacy-sse"
        with self.assertRaises(ManifestValidationError) as raised:
            normalize_manifest(data)
        self.assertIn(
            "legacy-ack-required", {issue.code for issue in raised.exception.issues}
        )
        data["integrations"]["web_mcp"]["legacy_sse_acknowledged"] = True
        self.assertEqual(
            normalize_manifest(data)["integrations"]["webMcp"]["legacySseAcknowledged"],
            True,
        )

    def test_tailscale_ingress_requires_attachment(self) -> None:
        data = fixture_data("valid/all-enabled.toml")
        data["integrations"]["tailscale"] = {"enabled": False}
        with self.assertRaises(ManifestValidationError) as raised:
            normalize_manifest(data)
        self.assertIn(
            "tailscale-required", {issue.code for issue in raised.exception.issues}
        )

    def test_catalog_manifests_all_validate_with_default_off(self) -> None:
        paths = sorted((ROOT / "src" / "cogs").glob("*/cog.toml"))
        self.assertEqual(len(paths), 109)
        for path in paths:
            with self.subTest(cog=path.parent.name):
                integrations = load_manifest(path)["integrations"]
                self.assertTrue(
                    all(value == {"enabled": False} for value in integrations.values())
                )

    def test_json_schema_is_valid_json_and_pins_security_constants(self) -> None:
        paths = sorted((ROOT / "schemas").glob("cog-integrations*.schema.json"))
        schemas = [json.loads(path.read_text()) for path in paths]
        encoded = json.dumps(schemas, sort_keys=True)
        self.assertEqual(len(schemas), 4)
        self.assertTrue(
            all(
                schema["$schema"] == "https://json-schema.org/draft/2020-12/schema"
                for schema in schemas
            )
        )
        self.assertIn("2025-11-25", encoded)
        self.assertIn("vite-production-v1", encoded)
        self.assertIn("tailscale-oauth-client", encoded)
        self.assertNotIn("buildCommand", encoded)
        self.assertNotIn("secretManagerRef", encoded)

    def test_cli_validates_catalog_and_emits_manifest(self) -> None:
        command = [sys.executable, "scripts/cog_integrations_cli.py"]
        checked = subprocess.run(
            command + ["validate", "--all"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(checked.returncode, 0, checked.stderr)
        self.assertIn("valid integration manifests: 109", checked.stdout)
        with tempfile.TemporaryDirectory() as output:
            emitted = subprocess.run(
                command
                + [
                    "emit",
                    str(FIXTURES / "valid" / "all-enabled.toml"),
                    "--out-dir",
                    output,
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(emitted.returncode, 0, emitted.stderr)
            metadata = json.loads(emitted.stdout)
            self.assertTrue(Path(metadata["manifestPath"]).is_file())
            self.assertRegex(metadata["digest"], r"^sha256:[a-f0-9]{64}$")

    def test_build_matrix_carries_version_and_rejects_path_traversal(self) -> None:
        valid = subprocess.run(
            [sys.executable, "scripts/cog-targets.py", "--matrix", "tailscale"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(valid.returncode, 0, valid.stderr)
        item = json.loads(valid.stdout)["include"][0]
        self.assertEqual(item["version"], "0.1.0")
        invalid = subprocess.run(
            [sys.executable, "scripts/cog-targets.py", "--matrix", "../tailscale"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(invalid.returncode, 0)
        self.assertIn("invalid cog id", invalid.stderr)

    def test_locked_website_profile_packages_deterministically(self) -> None:
        manifest = FIXTURES / "valid" / "all-enabled.toml"
        self.assertEqual(
            inspect_profile(manifest)["buildProfile"], "vite-production-v1"
        )
        with tempfile.TemporaryDirectory() as workspace:
            cog_dir = Path(workspace) / "cog"
            output = cog_dir / "web" / "dist"
            output.mkdir(parents=True)
            (output / "index.html").write_text("<!doctype html><title>Cog</title>")
            (output / "assets").mkdir()
            (output / "assets" / "app.js").write_text("console.log('cog');")
            first_dir = Path(workspace) / "first"
            second_dir = Path(workspace) / "second"
            first, first_digest, _ = package_website(manifest, cog_dir, first_dir)
            second, second_digest, _ = package_website(manifest, cog_dir, second_dir)
            self.assertEqual(first_digest, second_digest)
            self.assertEqual(first.name, second.name)
            self.assertEqual(first.read_bytes(), second.read_bytes())

    def test_website_bundle_rejects_symlinks(self) -> None:
        manifest = FIXTURES / "valid" / "all-enabled.toml"
        with tempfile.TemporaryDirectory() as workspace:
            cog_dir = Path(workspace) / "cog"
            output = cog_dir / "web" / "dist"
            output.mkdir(parents=True)
            (output / "index.html").write_text("safe")
            (output / "escape").symlink_to("/etc/passwd")
            with self.assertRaises(WebsiteBundleError):
                package_website(manifest, cog_dir, Path(workspace) / "out")

    def test_publish_workflows_are_wif_only_and_create_only(self) -> None:
        production = (ROOT / ".github" / "workflows" / "publish-cog.yml").read_text()
        batch = (ROOT / ".github" / "workflows" / "build-all-cogs.yml").read_text()
        staging = (
            ROOT / ".github" / "workflows" / "publish-cog-staging.yml"
        ).read_text()
        combined = production + batch + staging
        self.assertNotIn("credentials_json", combined)
        self.assertNotIn("GCP_COGNITUM_APPS_SA", combined)
        self.assertIn("id-token: write", combined)
        self.assertIn("--if-generation-match=0", combined)
        self.assertIn("cognitum-20260110-cog-release-stg", staging)
        self.assertNotIn("gs://cognitum-apps/staging/cogs/releases/", staging)
        self.assertIn("release-evidence-locations.json", staging)
        self.assertNotIn("\n  push:", staging)
        self.assertIn("environment: cogs-staging", staging)
        self.assertIn(
            "Refuse unsigned production publication (ADR-155 freeze)", production
        )
        self.assertIn("Refuse unsigned batch publication (ADR-155 freeze)", batch)
        self.assertNotIn("environment: cogs-production", production + batch)
        self.assertNotIn("GCP_COGS_PROD_", production + batch)
        self.assertNotIn("gs://cognitum-apps/cogs/releases/", production + batch)


if __name__ == "__main__":
    unittest.main()
