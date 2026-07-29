#!/usr/bin/env python3
"""Command-line interface for the stdlib Cog integration manifest contract."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from cog_integrations import COGS_DIR, ManifestValidationError, emit_manifest, load_manifest


def _print_error(error: ManifestValidationError) -> None:
    for issue in error.issues:
        print(f"{error.path}:{issue}", file=sys.stderr)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate = subparsers.add_parser("validate")
    validate.add_argument("manifests", nargs="*", type=Path)
    validate.add_argument("--all", action="store_true")
    emit = subparsers.add_parser("emit")
    emit.add_argument("manifest", type=Path)
    emit.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args(argv)

    if args.command == "validate":
        paths = sorted(COGS_DIR.glob("*/cog.toml")) if args.all else args.manifests
        if not paths:
            parser.error("validate requires manifest paths or --all")
        failures = 0
        for path in paths:
            try:
                load_manifest(path)
            except ManifestValidationError as error:
                failures += 1
                _print_error(error)
        if failures:
            print(f"invalid integration manifests: {failures}/{len(paths)}", file=sys.stderr)
            return 1
        print(f"valid integration manifests: {len(paths)}")
        return 0

    try:
        output, digest, checksum = emit_manifest(args.manifest, args.out_dir)
    except ManifestValidationError as error:
        _print_error(error)
        return 1
    print(json.dumps({
        "manifestPath": str(output),
        "digest": f"sha256:{digest}",
        "checksumPath": str(checksum),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
