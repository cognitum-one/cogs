#!/usr/bin/env python3
"""ADR-091 migration tool: rewrites a cog to use the shared
`cog-sensor-sources` crate. Idempotent; safe to re-run.

Per cog:
1. Adds `cog-sensor-sources = { path = "../../../crates/cog-sensor-sources" }`
   to [dependencies] in Cargo.toml (skipped if already present).
2. Replaces the body of `fn fetch_sensors() -> Result<serde_json::Value, String>`
   in src/main.rs with a single call to
   `cog_sensor_sources::fetch_sensors()`.
3. Bumps Cargo.toml version 1.0.x or 1.1.x -> 1.2.0 (skipped if already
   >= 1.2.0).

Usage:
    python migrate-cog-to-shared-sources.py <cog-dir> [<cog-dir>...]
    python migrate-cog-to-shared-sources.py --all   (every src/cogs/*)
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DEP_LINE = 'cog-sensor-sources = { path = "../../../crates/cog-sensor-sources" }'
NEW_BODY = '''fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}'''
COGS_ROOT = Path(__file__).resolve().parent.parent / "src" / "cogs"


def find_function_span(text: str, sig_re: re.Pattern) -> tuple[int, int] | None:
    """Find `fn ... {` ... matching `}`. Skips braces inside string/char
    literals and `// ...` / `/* ... */` comments. Returns (start, end)
    byte offsets or None.
    """
    m = sig_re.search(text)
    if not m:
        return None
    start = m.start()
    open_brace = text.index("{", m.end() - 1)
    depth = 0
    i = open_brace
    while i < len(text):
        c = text[i]
        # Line comment: skip to newline
        if c == "/" and i + 1 < len(text) and text[i + 1] == "/":
            nl = text.find("\n", i)
            i = (nl + 1) if nl != -1 else len(text)
            continue
        # Block comment: skip to */
        if c == "/" and i + 1 < len(text) and text[i + 1] == "*":
            end = text.find("*/", i + 2)
            i = (end + 2) if end != -1 else len(text)
            continue
        # String literal — handle escape \\ and \"
        if c == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        # Char literal — only handle simple/escaped single-byte
        if c == "'":
            # Skip if it's a lifetime like 'a (no closing single-quote follows quickly)
            # Heuristic: char lit is at most 4 chars: '\\x'
            close = text.find("'", i + 1)
            if 0 < close - i <= 4:
                i = close + 1
                continue
            # otherwise treat as lifetime; advance one char
            i += 1
            continue
        # Raw string literal r"..." or r#"..."#  (rare in our cogs but be safe)
        if c == "r" and i + 1 < len(text) and text[i + 1] in ('"', "#"):
            j = i + 1
            hashes = 0
            while j < len(text) and text[j] == "#":
                hashes += 1
                j += 1
            if j < len(text) and text[j] == '"':
                end_marker = '"' + "#" * hashes
                end = text.find(end_marker, j + 1)
                if end != -1:
                    i = end + len(end_marker)
                    continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return (start, i + 1)
        i += 1
    return None


def migrate_cargo_toml(path: Path) -> bool:
    """Add the dep + bump version. Returns True if changed."""
    text = path.read_text(encoding="utf-8")
    changed = False

    if "cog-sensor-sources" not in text:
        # Insert under [dependencies]. Match the line and add ours below.
        new_text, n = re.subn(
            r"^\[dependencies\]\r?\n",
            "[dependencies]\n" + DEP_LINE + "\n",
            text,
            count=1,
            flags=re.MULTILINE,
        )
        if n == 0:
            print(f"  WARN: no [dependencies] section in {path}")
            return False
        text = new_text
        changed = True

    # Bump version 1.0.x / 1.1.x -> 1.2.0 (idempotent — only if older)
    version_match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if version_match:
        ver = version_match.group(1)
        major_minor = tuple(int(x) for x in ver.split(".")[:2] + [0])[:2]
        if major_minor < (1, 2):
            text = re.sub(
                r'^version\s*=\s*"[^"]+"',
                'version = "1.2.0"',
                text,
                count=1,
                flags=re.MULTILINE,
            )
            changed = True

    if changed:
        path.write_text(text, encoding="utf-8")
    return changed


def migrate_main_rs(path: Path) -> bool:
    """Replace fetch_sensors() body. Returns True if changed."""
    text = path.read_text(encoding="utf-8")

    # Already migrated?
    if "cog_sensor_sources::fetch_sensors()" in text:
        return False

    sig_re = re.compile(
        r"^fn\s+fetch_sensors\s*\(\s*\)\s*->\s*Result\s*<\s*serde_json\s*::\s*Value\s*,\s*String\s*>\s*\{",
        re.MULTILINE,
    )
    span = find_function_span(text, sig_re)
    if not span:
        # No matching fetch_sensors — cog probably uses a different shape, skip
        return False

    start, end = span
    new_text = text[:start] + NEW_BODY + text[end:]

    # Drop the `use std::io::Read;` line if it's now orphaned (only used by old fetch_sensors).
    # Conservative: leave it. The compiler will warn but build.

    path.write_text(new_text, encoding="utf-8")
    return True


def migrate_cog(cog_dir: Path) -> dict[str, bool]:
    cargo = cog_dir / "Cargo.toml"
    main = cog_dir / "src" / "main.rs"
    if not cargo.exists() or not main.exists():
        return {"skipped": True}
    return {
        "cargo": migrate_cargo_toml(cargo),
        "main": migrate_main_rs(main),
    }


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("cogs", nargs="*", help="cog directory or name")
    p.add_argument("--all", action="store_true", help="migrate every cog under src/cogs/")
    p.add_argument("--dry-run", action="store_true")
    args = p.parse_args()

    targets: list[Path] = []
    if args.all:
        targets = sorted([d for d in COGS_ROOT.iterdir() if d.is_dir()])
    else:
        for c in args.cogs:
            cp = Path(c)
            if not cp.is_absolute() and not cp.exists():
                cp = COGS_ROOT / c
            targets.append(cp)

    if not targets:
        p.print_usage()
        return 2

    summary = {"changed": 0, "no-op": 0, "skipped": 0}
    for d in targets:
        if not d.is_dir():
            print(f"  skip (not a dir): {d.name}")
            summary["skipped"] += 1
            continue
        if args.dry_run:
            print(f"  would migrate: {d.name}")
            continue
        result = migrate_cog(d)
        if result.get("skipped"):
            print(f"  skip (no Cargo.toml/main.rs): {d.name}")
            summary["skipped"] += 1
        elif any(result.values()):
            tags = [k for k, v in result.items() if v]
            print(f"  migrated: {d.name}  ({', '.join(tags)})")
            summary["changed"] += 1
        else:
            print(f"  no-op: {d.name}")
            summary["no-op"] += 1

    print(f"\n  summary: {summary}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
