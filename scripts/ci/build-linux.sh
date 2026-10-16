#!/usr/bin/env bash
# Linux bundles (deb, rpm, AppImage) and the portable tarball, under release
# names. Runs in build:linux after setup-linux.sh, and in the ci-linux image
# for a local build (scripts/local/build-all.sh).
#
#   scripts/ci/build-linux.sh <version> <out-dir>
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
out="$2"
config="$(BUILD_VERSION="$version" scripts/ci/tauri-config.sh)"
pnpm tauri build --ci --bundles deb,rpm,appimage --config "$config"
scripts/ci/collect-linux.sh "$version" "$out"
