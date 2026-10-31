#!/usr/bin/env bash
# Linux bundles (deb, rpm, AppImage) and the portable tarball, under release
# names. Runs in build:linux after setup-linux.sh, and in the ci-linux image
# for a local build (cargo xtask build).
#
#   MWT_TAURI_CONFIG=<overlay> scripts/ci/build-linux.sh <version> <out-dir>
#
# MWT_TAURI_CONFIG is the Tauri config overlay (`cargo xtask tauri-config`).
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
out="$2"
config="${MWT_TAURI_CONFIG:?the Tauri config overlay (cargo xtask tauri-config)}"
# Bundles of an earlier build (the other channel names them differently)
# would be collected as well.
rm -rf "${CARGO_TARGET_DIR:-.rust-target}/release/bundle"
pnpm tauri build --ci --bundles deb,rpm,appimage --config "$config"
scripts/ci/collect-linux.sh "$version" "$out"
