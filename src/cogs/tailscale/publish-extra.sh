#!/usr/bin/env bash
# publish-extra.sh — tailscale-specific publish hook, sourced per-arch by the
# generic publish workflows (publish-cog.yml / build-all-cogs.yml). The cog is
# a thin Rust supervisor; the actual VPN ships as upstream tailscale/tailscaled
# binaries downloaded here and staged under out/extra/ for upload to
# gs://cognitum-apps/cogs/<gcs>/tailscale/<cog_version>/.
#
# The workflow exports: ARCH (armhf|aarch64), SUFFIX, GCS, COG_VERSION, OUT_DIR.
# Upstream version is pinned in cog.toml [upstream].tailscale_version.
set -euo pipefail

COG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TS_VERSION="$(python3 -c "import tomllib;print(tomllib.load(open('${COG_DIR}/cog.toml','rb'))['upstream']['tailscale_version'])")"

# Map our arch → upstream Tailscale arch token + the binary file suffix the
# seed registry expects (tailscaled-armhf / tailscaled-arm64).
case "${ARCH}" in
  armhf)   TS_ARCH="arm"   ; FILE_SUFFIX="armhf" ;;
  aarch64) TS_ARCH="arm64" ; FILE_SUFFIX="arm64" ;;
  *) echo "publish-extra.sh: unsupported ARCH=${ARCH}" >&2; exit 1 ;;
esac

STAGE="${OUT_DIR}/extra/tailscale/${COG_VERSION}"
mkdir -p "${STAGE}"

TGZ="/tmp/tailscale_${TS_VERSION}_${TS_ARCH}.tgz"
URL="https://pkgs.tailscale.com/stable/tailscale_${TS_VERSION}_${TS_ARCH}.tgz"
echo "Downloading ${URL}"
curl -sLfo "${TGZ}" "${URL}"
tar -xzf "${TGZ}" -C /tmp
SRC="/tmp/tailscale_${TS_VERSION}_${TS_ARCH}"

cp "${SRC}/tailscaled" "${STAGE}/tailscaled-${FILE_SUFFIX}"
cp "${SRC}/tailscale"  "${STAGE}/tailscale-${FILE_SUFFIX}"
chmod +x "${STAGE}/tailscaled-${FILE_SUFFIX}" "${STAGE}/tailscale-${FILE_SUFFIX}"

echo "Staged upstream Tailscale ${TS_VERSION} (${TS_ARCH}) for cog ${COG_VERSION}:"
ls -lh "${STAGE}"
