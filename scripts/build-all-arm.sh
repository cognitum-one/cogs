#!/usr/bin/env bash
# build-all-arm.sh — cross-compile every cog under src/cogs/ for the Pi 5
# appliance/seed fleet (armv7-unknown-linux-gnueabihf / armhf) and collect the
# stripped binaries into dist/armv7/.
#
# Per ADR-001 §6: "registered automatically by being in src/cogs/ — the build
# script loops over the directory." Per README: Docker-based armhf cross-compile.
#
# This implementation prefers the natively-installed armhf cross toolchain
# (rustup target + gcc-arm-linux-gnueabihf) which is far faster than a
# per-cog Docker build, and falls back to nothing — if the native toolchain
# is missing it errors out. The release profile (strip=true) is set per-cog
# in each cog's Cargo.toml.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COGS_DIR="$REPO_ROOT/src/cogs"
TARGET="armv7-unknown-linux-gnueabihf"
DIST_DIR="$REPO_ROOT/dist/armv7"
LOG="$REPO_ROOT/dist/build.log"
FAILURES="$REPO_ROOT/dist/failures.txt"

# The host shell exports RUSTFLAGS=-C link-arg=-fuse-ld=mold which cannot
# cross-link for ARM. Clear it so arm-linux-gnueabihf-gcc uses its bundled ld.
unset RUSTFLAGS
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc

mkdir -p "$DIST_DIR"
: > "$LOG"
: > "$FAILURES"

# Sanity: native cross toolchain present.
if ! command -v arm-linux-gnueabihf-gcc >/dev/null 2>&1; then
  echo "FATAL: arm-linux-gnueabihf-gcc not found (install gcc-arm-linux-gnueabihf)" | tee -a "$LOG"
  exit 2
fi
if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  echo "FATAL: rust target $TARGET not installed (rustup target add $TARGET)" | tee -a "$LOG"
  exit 2
fi

total=0
built=0
for cog_dir in "$COGS_DIR"/*/; do
  [ -f "$cog_dir/Cargo.toml" ] || continue
  cog="$(basename "$cog_dir")"
  total=$((total + 1))

  # Resolve the binary name from [[bin]] name, else package name.
  bin_name="$(awk '/^\[\[bin\]\]/{inbin=1} inbin&&/^name *=/{gsub(/[" ]/,"",$0);split($0,a,"=");print a[2];exit}' "$cog_dir/Cargo.toml")"
  if [ -z "$bin_name" ]; then
    bin_name="$(awk '/^\[package\]/{inpkg=1} inpkg&&/^name *=/{gsub(/[" ]/,"",$0);split($0,a,"=");print a[2];exit}' "$cog_dir/Cargo.toml")"
  fi
  [ -z "$bin_name" ] && bin_name="cog-$cog"

  # The appliance cog supervisor installs the artifact named by cog.toml's
  # `binary` field (e.g. "cog-<cog>-arm"). Emit the dist file under THAT name so
  # an install needs no per-cog override (ADR-019 Stage B: a cog-<cog> vs
  # cog-<cog>-arm mismatch forced manual binary_url overrides for several cogs).
  dist_name="$(awk -F'=' '/^binary *=/{gsub(/[" ]/,"",$2);print $2;exit}' "$cog_dir/cog.toml" 2>/dev/null)"
  [ -z "$dist_name" ] && dist_name="cog-$cog-arm"

  if ( cd "$cog_dir" && cargo build --release --target "$TARGET" ) >>"$LOG" 2>&1; then
    out="$cog_dir/target/$TARGET/release/$bin_name"
    if [ -f "$out" ]; then
      cp "$out" "$DIST_DIR/$dist_name"
      arm-linux-gnueabihf-strip "$DIST_DIR/$dist_name" 2>/dev/null || true
      built=$((built + 1))
      echo "OK   $cog" | tee -a "$LOG"
    else
      echo "FAIL $cog (no binary at $out)" | tee -a "$LOG"
      echo "$cog: build succeeded but binary $bin_name not found" >> "$FAILURES"
    fi
  else
    echo "FAIL $cog (cargo build failed — see $LOG)" | tee -a "$LOG"
    echo "$cog: cargo build failed" >> "$FAILURES"
  fi
done

echo "=================================================="
echo "target=$TARGET  total=$total  built=$built  failed=$((total - built))"
echo "dist=$DIST_DIR"
[ -s "$FAILURES" ] && { echo "failures:"; cat "$FAILURES"; }
exit 0
