#!/usr/bin/env python3
"""Inspect and deterministically package a locked Cog website build output."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import tarfile
import tempfile
from pathlib import Path
from typing import BinaryIO

from cog_integrations import load_manifest

MAX_FILES = 10_000
MAX_BYTES = 100 * 1024 * 1024


class WebsiteBundleError(ValueError):
    pass


def inspect_profile(manifest_path: Path) -> dict[str, object]:
    website = load_manifest(manifest_path)["integrations"]["website"]
    if not website["enabled"]:
        return {"enabled": False, "kind": "none"}
    artifact = website["artifact"]
    if artifact["kind"] == "oci-image":
        return {
            "enabled": True,
            "kind": "oci-image",
            "image": artifact["image"],
        }
    return {
        "enabled": True,
        "kind": "static-build",
        "buildProfile": artifact["buildProfile"],
        "outputDirectory": artifact["outputDirectory"],
    }


def _safe_output(cog_dir: Path, relative: str) -> Path:
    root = cog_dir.resolve(strict=True)
    unresolved = root / relative
    if unresolved.is_symlink():
        raise WebsiteBundleError("website output directory cannot be a symlink")
    output = unresolved.resolve(strict=True)
    if not output.is_dir() or not output.is_relative_to(root):
        raise WebsiteBundleError("website output must be a directory inside the cog")
    return output


def _entries(output: Path) -> list[Path]:
    entries = sorted(output.rglob("*"), key=lambda path: path.relative_to(output).as_posix())
    files = 0
    total_bytes = 0
    for entry in entries:
        if entry.is_symlink():
            raise WebsiteBundleError(f"website output cannot contain symlink: {entry}")
        if entry.is_file():
            files += 1
            total_bytes += entry.stat().st_size
        elif not entry.is_dir():
            raise WebsiteBundleError(f"website output contains unsupported entry: {entry}")
    if files == 0:
        raise WebsiteBundleError("website output contains no files")
    if files > MAX_FILES:
        raise WebsiteBundleError(f"website output exceeds {MAX_FILES} files")
    if total_bytes > MAX_BYTES:
        raise WebsiteBundleError(f"website output exceeds {MAX_BYTES} bytes")
    return entries


def _tar_info(name: str, source: Path) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    if source.is_dir():
        info.type = tarfile.DIRTYPE
        info.mode = 0o755
        info.size = 0
    else:
        info.type = tarfile.REGTYPE
        info.mode = 0o644
        info.size = source.stat().st_size
    return info


def _write_archive(output: Path, entries: list[Path], destination: BinaryIO) -> None:
    with gzip.GzipFile(filename="", mode="wb", fileobj=destination, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.GNU_FORMAT) as archive:
            root_info = tarfile.TarInfo("site")
            root_info.type = tarfile.DIRTYPE
            root_info.mode = 0o755
            root_info.uid = root_info.gid = root_info.mtime = 0
            archive.addfile(root_info)
            for entry in entries:
                relative = entry.relative_to(output).as_posix()
                info = _tar_info(f"site/{relative}", entry)
                if entry.is_file():
                    with entry.open("rb") as stream:
                        archive.addfile(info, stream)
                else:
                    archive.addfile(info)


def package_website(manifest_path: Path, cog_dir: Path, out_dir: Path) -> tuple[Path, str, Path]:
    manifest = load_manifest(manifest_path)
    profile = inspect_profile(manifest_path)
    if profile["kind"] != "static-build":
        raise WebsiteBundleError("only enabled static-build websites produce a bundle")
    output = _safe_output(cog_dir, str(profile["outputDirectory"]))
    entries = _entries(output)
    out_dir.mkdir(parents=True, exist_ok=True)
    temporary = tempfile.NamedTemporaryFile(prefix=".website-", suffix=".tar.gz", dir=out_dir, delete=False)
    temporary_path = Path(temporary.name)
    try:
        with temporary:
            _write_archive(output, entries, temporary)
        digest = hashlib.sha256(temporary_path.read_bytes()).hexdigest()
        cog_id = manifest["cog"]["id"]
        name = f"cog-{cog_id}-website-v1-sha256-{digest}.tar.gz"
        bundle = out_dir / name
        os.replace(temporary_path, bundle)
        checksum = bundle.with_suffix(bundle.suffix + ".sha256")
        checksum.write_text(f"{digest}  {name}\n", encoding="utf-8")
        return bundle, digest, checksum
    finally:
        temporary_path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    inspect = subparsers.add_parser("inspect")
    inspect.add_argument("manifest", type=Path)
    package = subparsers.add_parser("package")
    package.add_argument("manifest", type=Path)
    package.add_argument("--cog-dir", type=Path, required=True)
    package.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "inspect":
            print(json.dumps(inspect_profile(args.manifest), sort_keys=True))
            return 0
        bundle, digest, checksum = package_website(args.manifest, args.cog_dir, args.out_dir)
    except (OSError, WebsiteBundleError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")
    print(json.dumps({
        "bundlePath": str(bundle),
        "digest": f"sha256:{digest}",
        "checksumPath": str(checksum),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
