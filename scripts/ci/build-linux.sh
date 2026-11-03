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
# Updater artifacts (<bundle>.sig) need TAURI_SIGNING_PRIVATE_KEY: tauri fails the build when
# createUpdaterArtifacts is on and the key is missing, so a build without the key (local, a fork)
# simply makes no .sig files, and the release has no updater entry for Linux.
sign_config="$(mktemp --suffix=.json)"
echo '{"bundle":{"createUpdaterArtifacts":true}}' >"$sign_config"
sign=()
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then sign=(--config "$sign_config"); fi
if [[ "$version" == *-* ]]; then
  # A pre-release X.Y.Z-N: RPM forbids `-` in Version (tauri-cli 2.12.4 checks it), so the
  # rpm is packed from the same binary as Version X.Y.Z, Release 0.N, which sorts before
  # the X.Y.Z release (Release 1). deb and AppImage take the semver as it is.
  pnpm tauri build --ci --bundles deb,appimage --config "$config" "${sign[@]}"
  rpm_config="$(mktemp --suffix=.json)"
  printf '{"version":"%s","bundle":{"linux":{"rpm":{"release":"0.%s"}}}}\n' \
    "${version%%-*}" "${version#*-}" >"$rpm_config"
  pnpm tauri bundle --ci --bundles rpm --config "$config" --config "$rpm_config"
  rm -f "$rpm_config"
else
  pnpm tauri build --ci --bundles deb,rpm,appimage --config "$config" "${sign[@]}"
fi
rm -f "$sign_config"
scripts/ci/collect-linux.sh "$version" "$out"
