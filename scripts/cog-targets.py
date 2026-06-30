#!/usr/bin/env python3
"""cog-targets.py — resolve each cog's build targets from its cog.toml
`hardware_requirement`, the single source of truth for which device(s) a cog
ships to. Used by both publish workflows (publish-cog.yml per-cog dispatch and
build-all-cogs.yml umbrella batch) so the arch gating lives in one place.

Device → arch mapping (ADR-095 hardware envelope):
  pi-zero-2w   → armhf   (Pi Zero 2 W seed,  armv7-unknown-linux-gnueabihf)
  v0-appliance → aarch64 (Pi 5 v0 hub,       aarch64-unknown-linux-gnu)

A cog with no `hardware_requirement` (or an empty list) defaults to BOTH — the
catalog is CSI/audio/sensor-DSP cogs that run on the edge seed and the more
powerful v0 alike. Declare a single device to opt a cog out of an arch (e.g. a
future Hailo-NPU-only vision cog → `["v0-appliance"]`).

Usage:
  cog-targets.py --matrix [cog ...]   # GitHub Actions matrix JSON on stdout
  cog-targets.py --arches <cog>       # space-separated arch names for one cog
"""
from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path

COGS_DIR = Path(__file__).resolve().parent.parent / "src" / "cogs"

DEVICE_ARCH = {
    "pi-zero-2w": "armhf",
    "v0-appliance": "aarch64",
}

# Per-arch cross-compile recipe. `gcs` is the gs://cognitum-apps/cogs/<gcs>/
# prefix; `suffix` is appended to the cargo bin name (cog-<id>) at upload time.
ARCHES = {
    "armhf": {
        "triple": "armv7-unknown-linux-gnueabihf",
        "suffix": "arm",
        "gcs": "arm",
        "linker": "arm-linux-gnueabihf-gcc",
        "strip": "arm-linux-gnueabihf-strip",
        "apt": "gcc-arm-linux-gnueabihf libc6-dev-armhf-cross",
        "rustflags": "",
    },
    "aarch64": {
        "triple": "aarch64-unknown-linux-gnu",
        "suffix": "aarch64",
        "gcs": "arm64",
        "linker": "aarch64-linux-gnu-gcc",
        "strip": "aarch64-linux-gnu-strip",
        # Pi 5 is Cortex-A76 — match the appliance release-binary build flags.
        "apt": "gcc-aarch64-linux-gnu libc6-dev-arm64-cross",
        "rustflags": "-C target-cpu=cortex-a76",
    },
}


def arches_for(cog: str) -> list[str]:
    """Arch names a cog builds for, ordered armhf then aarch64."""
    toml_path = COGS_DIR / cog / "cog.toml"
    if not toml_path.is_file():
        raise SystemExit(f"error: no cog.toml for '{cog}' ({toml_path})")
    with toml_path.open("rb") as f:
        data = tomllib.load(f)
    hw = data.get("cog", {}).get("hardware_requirement")
    if isinstance(hw, str):
        hw = [hw]
    if not hw:  # missing or empty → both devices
        hw = list(DEVICE_ARCH.keys())
    arches = []
    for dev in hw:
        arch = DEVICE_ARCH.get(dev)
        if arch is None:
            print(f"warning: {cog}: unknown device '{dev}' in "
                  f"hardware_requirement — skipping", file=sys.stderr)
            continue
        if arch not in arches:
            arches.append(arch)
    # deterministic order
    return [a for a in ("armhf", "aarch64") if a in arches]


def all_cogs() -> list[str]:
    return sorted(d.name for d in COGS_DIR.iterdir()
                  if (d / "cog.toml").is_file())


def build_matrix(cogs: list[str]) -> dict:
    include = []
    for cog in cogs:
        for arch in arches_for(cog):
            a = ARCHES[arch]
            include.append({
                "cog": cog,
                "arch": arch,
                "triple": a["triple"],
                "suffix": a["suffix"],
                "gcs": a["gcs"],
                "linker": a["linker"],
                "strip": a["strip"],
                "apt": a["apt"],
                "rustflags": a["rustflags"],
            })
    return {"include": include}


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    mode = argv[0]
    if mode == "--arches":
        if len(argv) != 2:
            raise SystemExit("usage: cog-targets.py --arches <cog>")
        print(" ".join(arches_for(argv[1])))
        return 0
    if mode == "--matrix":
        cogs = argv[1:] or all_cogs()
        print(json.dumps(build_matrix(cogs)))
        return 0
    raise SystemExit(f"unknown mode: {mode}")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
