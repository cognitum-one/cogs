#!/usr/bin/env bash
# build-all-arm64.sh — cross-compile cogs under src/cogs/ for the Pi 5
# **appliance** (aarch64-unknown-linux-gnu) and collect stripped binaries into
# dist/aarch64/. Companion to build-all-arm.sh (armv7/seed). #167: the v0
# appliance is aarch64, so it needs aarch64 cog builds — the armhf binaries the
# catalog ships can't run on it.
#
# Usage:
#   scripts/build-all-arm64.sh                # build every cog under src/cogs/
#   scripts/build-all-arm64.sh health-monitor fall-detect   # build a subset
#
# Binaries are emitted under the cog.toml `binary` name with the -arm suffix
# rewritten to -aarch64 (e.g. cog-health-monitor-aarch64), matching the GCS
# layout gs://cognitum-apps/cogs/arm64/ and the gateway's appliance install path.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COGS_DIR="$REPO_ROOT/src/cogs"
TARGET="aarch64-unknown-linux-gnu"
DIST_DIR="$REPO_ROOT/dist/aarch64"
LOG="$REPO_ROOT/dist/build-arm64.log"
FAILURES="$REPO_ROOT/dist/failures-arm64.txt"

# The host shell may export RUSTFLAGS with a non-cross linker (mold/lld); clear
# it so the aarch64 gcc uses its bundled ld (same reason as build-all-arm.sh).
unset RUSTFLAGS
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
# Pi 5 is Cortex-A76; match the appliance release-binary build flags.
export RUSTFLAGS="-C target-cpu=cortex-a76"

mkdir -p "$DIST_DIR"
: > "$LOG"
: > "$FAILURES"

command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 || {
  echo "FATAL: aarch64-linux-gnu-gcc not found (brew install aarch64-unknown-linux-gnu or apt gcc-aarch64-linux-gnu)" | tee -a "$LOG"; exit 2; }
rustup target list --installed 2>/dev/null | grep -qx "$TARGET" || {
  echo "FATAL: rust target $TARGET not installed (rustup target add $TARGET)" | tee -a "$LOG"; exit 2; }
STRIP="$(command -v aarch64-linux-gnu-strip || true)"

# Cog list: explicit args, else every dir under src/cogs/.
if [ "$#" -gt 0 ]; then
  cogs=("$@")
else
  cogs=()
  for d in "$COGS_DIR"/*/; do cogs+=("$(basename "$d")"); done
fi

total=0 built=0
for cog in "${cogs[@]}"; do
  cog_dir="$COGS_DIR/$cog"
  [ -f "$cog_dir/Cargo.toml" ] || { echo "SKIP $cog (no Cargo.toml)" | tee -a "$LOG"; continue; }
  total=$((total + 1))

  # Binary name from [[bin]] name, else package name, else cog-<cog>.
  bin_name="$(awk '/^\[\[bin\]\]/{inbin=1} inbin&&/^name *=/{gsub(/[" ]/,"",$0);split($0,a,"=");print a[2];exit}' "$cog_dir/Cargo.toml")"
  [ -z "$bin_name" ] && bin_name="$(awk '/^\[package\]/{inpkg=1} inpkg&&/^name *=/{gsub(/[" ]/,"",$0);split($0,a,"=");print a[2];exit}' "$cog_dir/Cargo.toml")"
  [ -z "$bin_name" ] && bin_name="cog-$cog"

  # Dist name = cog.toml `binary` with -arm rewritten to -aarch64; else cog-<cog>-aarch64.
  dist_name="$(awk -F'=' '/^binary *=/{gsub(/[" ]/,"",$2);print $2;exit}' "$cog_dir/cog.toml" 2>/dev/null)"
  if [ -n "$dist_name" ]; then
    dist_name="${dist_name%-arm}-aarch64"
  else
    dist_name="cog-$cog-aarch64"
  fi

  if ( cd "$cog_dir" && cargo build --release --target "$TARGET" ) >>"$LOG" 2>&1; then
    out="$cog_dir/target/$TARGET/release/$bin_name"
    if [ -f "$out" ]; then
      cp "$out" "$DIST_DIR/$dist_name"
      [ -n "$STRIP" ] && "$STRIP" "$DIST_DIR/$dist_name" 2>/dev/null || true
      built=$((built + 1))
      echo "OK   $cog -> $dist_name" | tee -a "$LOG"
    else
      echo "FAIL $cog (no binary at $out)" | tee -a "$LOG"; echo "$cog: binary $bin_name not found" >> "$FAILURES"
    fi
  else
    echo "FAIL $cog (cargo build failed — see $LOG)" | tee -a "$LOG"; echo "$cog: cargo build failed" >> "$FAILURES"
  fi
done

echo "=================================================="
echo "target=$TARGET  total=$total  built=$built  failed=$((total - built))"
echo "dist=$DIST_DIR"
[ -s "$FAILURES" ] && { echo "failures:"; cat "$FAILURES"; }
exit 0
