#!/usr/bin/env python3
"""Validate local Cog release schemas, references, and public fixtures.

This deliberately uses only the Python standard library and the repository's
strict release validators. CI must not fetch an unpinned JSON Schema runtime in
order to decide whether the release contract is internally coherent.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any
from urllib.parse import unquote, urldefrag, urljoin, urlsplit

ROOT = Path(__file__).resolve().parent.parent
SCHEMA_DIR = ROOT / "schemas"
EXPECTED_DIALECT = "https://json-schema.org/draft/2020-12/schema"

sys.path.insert(0, str(ROOT / "scripts"))

from cog_release_provenance import verify  # noqa: E402
from cog_release_provenance_lib import (  # noqa: E402
    read_json,
    validate_policy,
    validate_release,
)


class SchemaCheckError(ValueError):
    """A local schema, reference, or fixture is invalid."""


ANNOTATIONS = {"$schema", "$id", "title", "description", "$defs"}
VALIDATION_KEYWORDS = {
    "$ref",
    "type",
    "const",
    "enum",
    "allOf",
    "anyOf",
    "oneOf",
    "if",
    "then",
    "else",
    "required",
    "properties",
    "additionalProperties",
    "propertyNames",
    "minProperties",
    "maxProperties",
    "items",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
}


def _load_schemas() -> tuple[dict[str, dict[str, Any]], dict[Path, dict[str, Any]]]:
    by_id: dict[str, dict[str, Any]] = {}
    by_path: dict[Path, dict[str, Any]] = {}
    for path in sorted(SCHEMA_DIR.glob("*.schema.json")):
        try:
            schema = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
            raise SchemaCheckError(f"{path}: invalid UTF-8 JSON") from error
        if not isinstance(schema, dict):
            raise SchemaCheckError(f"{path}: schema root must be an object")
        if schema.get("$schema") != EXPECTED_DIALECT:
            raise SchemaCheckError(f"{path}: unsupported or missing schema dialect")
        schema_id = schema.get("$id")
        parsed = urlsplit(schema_id) if isinstance(schema_id, str) else None
        if not parsed or parsed.scheme != "https" or not parsed.netloc:
            raise SchemaCheckError(f"{path}: $id must be one absolute HTTPS URI")
        if schema_id in by_id:
            raise SchemaCheckError(f"{path}: duplicate schema $id {schema_id}")
        by_id[schema_id] = schema
        by_path[path] = schema
    return by_id, by_path


def _inspect_schema(schema: Any, path: str = "$") -> list[str]:
    """Reject unsupported schema syntax and return every reachable reference."""

    if isinstance(schema, bool):
        return []
    if not isinstance(schema, dict):
        raise SchemaCheckError(f"{path}: schema node must be an object or boolean")
    unsupported = set(schema) - ANNOTATIONS - VALIDATION_KEYWORDS
    if unsupported:
        raise SchemaCheckError(
            f"{path}: unsupported schema keywords {sorted(unsupported)}"
        )
    references: list[str] = []
    if "$ref" in schema:
        reference = schema["$ref"]
        if not isinstance(reference, str):
            raise SchemaCheckError(f"{path}: $ref must be a string")
        references.append(reference)
    for keyword in ("$defs", "properties"):
        children = schema.get(keyword, {})
        if not isinstance(children, dict):
            raise SchemaCheckError(f"{path}: {keyword} must be an object")
        for name, child in children.items():
            references.extend(_inspect_schema(child, f"{path}.{keyword}.{name}"))
    for keyword in (
        "additionalProperties",
        "propertyNames",
        "items",
        "if",
        "then",
        "else",
    ):
        if keyword in schema:
            references.extend(_inspect_schema(schema[keyword], f"{path}.{keyword}"))
    for keyword in ("allOf", "anyOf", "oneOf"):
        if keyword not in schema:
            continue
        branches = schema[keyword]
        if not isinstance(branches, list) or not branches:
            raise SchemaCheckError(f"{path}: {keyword} must be a non-empty array")
        for index, branch in enumerate(branches):
            references.extend(_inspect_schema(branch, f"{path}.{keyword}[{index}]"))
    return references


def _pointer(document: Any, fragment: str, reference: str) -> Any:
    if not fragment:
        return document
    if not fragment.startswith("/"):
        raise SchemaCheckError(f"unsupported non-pointer fragment in $ref {reference}")
    current = document
    for raw in fragment.removeprefix("/").split("/"):
        segment = unquote(raw).replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and segment in current:
            current = current[segment]
            continue
        if isinstance(current, list) and segment.isdigit():
            index = int(segment)
            if index < len(current):
                current = current[index]
                continue
        raise SchemaCheckError(f"unresolved JSON pointer in $ref {reference}")
    return current


def _resolve(
    reference: str,
    base_uri: str,
    registry: dict[str, dict[str, Any]],
) -> tuple[Any, str]:
    absolute = urljoin(base_uri, reference)
    document_uri, fragment = urldefrag(absolute)
    document = registry.get(document_uri)
    if document is None:
        raise SchemaCheckError(f"$ref does not resolve to a local schema: {reference}")
    return _pointer(document, fragment, reference), document_uri


def _same_json(left: Any, right: Any) -> bool:
    if isinstance(left, bool) or isinstance(right, bool):
        return type(left) is type(right) and left == right
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return left == right
    return type(left) is type(right) and left == right


def _type_matches(instance: Any, expected: str) -> bool:
    return {
        "null": instance is None,
        "boolean": isinstance(instance, bool),
        "object": isinstance(instance, dict),
        "array": isinstance(instance, list),
        "string": isinstance(instance, str),
        "integer": isinstance(instance, int) and not isinstance(instance, bool),
        "number": isinstance(instance, (int, float)) and not isinstance(instance, bool),
    }.get(expected, False)


def _validate(
    instance: Any,
    schema: Any,
    *,
    base_uri: str,
    registry: dict[str, dict[str, Any]],
    path: str = "$",
) -> list[str]:
    if schema is True:
        return []
    if schema is False:
        return [f"{path}: false schema rejects value"]
    if not isinstance(schema, dict):
        return [f"{path}: schema node is not an object or boolean"]
    unsupported = set(schema) - ANNOTATIONS - VALIDATION_KEYWORDS
    if unsupported:
        return [f"{path}: unsupported schema keywords {sorted(unsupported)}"]

    errors: list[str] = []
    if "$ref" in schema:
        reference = schema["$ref"]
        if not isinstance(reference, str):
            return [f"{path}: $ref must be a string"]
        try:
            target, target_base = _resolve(reference, base_uri, registry)
        except SchemaCheckError as error:
            return [f"{path}: {error}"]
        errors.extend(
            _validate(
                instance,
                target,
                base_uri=target_base,
                registry=registry,
                path=path,
            )
        )

    expected_type = schema.get("type")
    if expected_type is not None:
        choices = expected_type if isinstance(expected_type, list) else [expected_type]
        if not choices or any(not isinstance(item, str) for item in choices):
            errors.append(f"{path}: schema type is malformed")
        elif not any(_type_matches(instance, item) for item in choices):
            errors.append(
                f"{path}: expected type {choices}, got {type(instance).__name__}"
            )
            return errors

    if "const" in schema and not _same_json(instance, schema["const"]):
        errors.append(f"{path}: value does not equal const")
    if "enum" in schema and not any(
        _same_json(instance, item) for item in schema["enum"]
    ):
        errors.append(f"{path}: value is not in enum")

    for name in ("allOf", "anyOf", "oneOf"):
        if name not in schema:
            continue
        branches = schema[name]
        if not isinstance(branches, list) or not branches:
            errors.append(f"{path}: {name} must be a non-empty array")
            continue
        results = [
            _validate(
                instance,
                branch,
                base_uri=base_uri,
                registry=registry,
                path=path,
            )
            for branch in branches
        ]
        matches = sum(not result for result in results)
        if name == "allOf":
            for result in results:
                errors.extend(result)
        elif name == "anyOf" and matches == 0:
            errors.append(f"{path}: no anyOf branch matched")
        elif name == "oneOf" and matches != 1:
            errors.append(f"{path}: expected one oneOf match, got {matches}")

    if "if" in schema:
        condition = _validate(
            instance,
            schema["if"],
            base_uri=base_uri,
            registry=registry,
            path=path,
        )
        selected = "then" if not condition else "else"
        if selected in schema:
            errors.extend(
                _validate(
                    instance,
                    schema[selected],
                    base_uri=base_uri,
                    registry=registry,
                    path=path,
                )
            )

    if isinstance(instance, dict):
        required = schema.get("required", [])
        if not isinstance(required, list):
            errors.append(f"{path}: required must be an array")
        else:
            for key in required:
                if key not in instance:
                    errors.append(f"{path}: missing required property {key}")
        properties = schema.get("properties", {})
        if not isinstance(properties, dict):
            errors.append(f"{path}: properties must be an object")
            properties = {}
        for key, child in properties.items():
            if key in instance:
                errors.extend(
                    _validate(
                        instance[key],
                        child,
                        base_uri=base_uri,
                        registry=registry,
                        path=f"{path}.{key}",
                    )
                )
        additional = schema.get("additionalProperties", True)
        for key in set(instance) - set(properties):
            if additional is False:
                errors.append(f"{path}: additional property {key} is forbidden")
            elif isinstance(additional, (dict, bool)):
                errors.extend(
                    _validate(
                        instance[key],
                        additional,
                        base_uri=base_uri,
                        registry=registry,
                        path=f"{path}.{key}",
                    )
                )
        if "propertyNames" in schema:
            for key in instance:
                errors.extend(
                    _validate(
                        key,
                        schema["propertyNames"],
                        base_uri=base_uri,
                        registry=registry,
                        path=f"{path}.<propertyName>",
                    )
                )
        if len(instance) < schema.get("minProperties", 0):
            errors.append(f"{path}: too few properties")
        if "maxProperties" in schema and len(instance) > schema["maxProperties"]:
            errors.append(f"{path}: too many properties")

    if isinstance(instance, list):
        if "items" in schema:
            for index, item in enumerate(instance):
                errors.extend(
                    _validate(
                        item,
                        schema["items"],
                        base_uri=base_uri,
                        registry=registry,
                        path=f"{path}[{index}]",
                    )
                )
        if len(instance) < schema.get("minItems", 0):
            errors.append(f"{path}: too few items")
        if "maxItems" in schema and len(instance) > schema["maxItems"]:
            errors.append(f"{path}: too many items")
        if schema.get("uniqueItems") is True:
            encoded = [
                json.dumps(item, sort_keys=True, separators=(",", ":"))
                for item in instance
            ]
            if len(encoded) != len(set(encoded)):
                errors.append(f"{path}: duplicate array items")

    if isinstance(instance, str):
        if len(instance) < schema.get("minLength", 0):
            errors.append(f"{path}: string is too short")
        if "maxLength" in schema and len(instance) > schema["maxLength"]:
            errors.append(f"{path}: string is too long")
        if "pattern" in schema and not re.search(schema["pattern"], instance):
            errors.append(f"{path}: string does not match pattern")
        if schema.get("format") == "uri":
            parsed = urlsplit(instance)
            if not parsed.scheme:
                errors.append(f"{path}: string is not an absolute URI")
        elif "format" in schema:
            errors.append(f"{path}: unsupported format {schema['format']}")

    if isinstance(instance, (int, float)) and not isinstance(instance, bool):
        if "minimum" in schema and instance < schema["minimum"]:
            errors.append(f"{path}: number is below minimum")
        if "maximum" in schema and instance > schema["maximum"]:
            errors.append(f"{path}: number is above maximum")
    return errors


def check() -> None:
    registry, by_path = _load_schemas()
    release_paths = sorted(SCHEMA_DIR.glob("cognitum.cog.release-*.schema.json"))
    integration_paths = sorted(SCHEMA_DIR.glob("cog-integrations*.schema.json"))
    if len(release_paths) != 4 or len(integration_paths) != 4:
        raise SchemaCheckError(
            "expected exactly four release and four integration schemas"
        )

    for path, schema in by_path.items():
        base_uri = schema["$id"]
        for reference in _inspect_schema(schema):
            if path in release_paths and not reference.startswith("#"):
                parsed = urlsplit(reference)
                if parsed.scheme != "https" or not parsed.netloc:
                    raise SchemaCheckError(
                        f"{path}: cross-file $ref must be absolute HTTPS: {reference}"
                    )
            _resolve(reference, base_uri, registry)

    fixtures = (
        (
            "https://schemas.cognitum.one/cognitum.cog.release-policy.v1.schema.json",
            ROOT / "src/cogs/anomaly-detect/release-policy.json",
        ),
        (
            "https://schemas.cognitum.one/cognitum.cog.release-record.v1.schema.json",
            ROOT / "tests/fixtures/cog-release/signed-release.json",
        ),
        (
            "https://schemas.cognitum.one/cognitum.cog.release-trust.v1.schema.json",
            ROOT / "tests/fixtures/cog-release/release-trust-registry.json",
        ),
    )
    for schema_id, fixture_path in fixtures:
        fixture = read_json(fixture_path)
        errors = _validate(
            fixture,
            registry[schema_id],
            base_uri=schema_id,
            registry=registry,
        )
        if errors:
            raise SchemaCheckError(
                f"{fixture_path}: schema validation failed: {'; '.join(errors[:8])}"
            )

    policy = read_json(fixtures[0][1])
    release = read_json(fixtures[1][1])
    validate_policy(policy)
    validate_release(release, signed=True)
    verify(argparse.Namespace(release=fixtures[1][1], registry=fixtures[2][1]))
    print(
        "validated 8 local schemas, all references, ratified policy, "
        "and public signed fixtures"
    )


def main() -> int:
    try:
        check()
    except (OSError, SchemaCheckError, ValueError) as error:
        print(f"release schema check error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
