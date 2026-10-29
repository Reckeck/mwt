#!/usr/bin/env bash
# Write the Tauri config overlay of a CI build — the version from version.sh
# (BUILD_VERSION), the Android versionCode and the channel's identifier, name
# binary and icons (channel.sh) — and print its path, for
# `tauri build --config <path>`. A file, not inline JSON: it survives every
# shell's quoting (build-windows.ps1 writes the same file itself).
#
#   scripts/ci/tauri-config.sh [--android]   # --android: the Android identifier
#
# Two variables for a local build (cargo xtask build), unset in CI:
#   MWT_PREBUILT_FRONTEND=1  dist/ is already built: an empty beforeBuildCommand keeps
#                            tauri from running `pnpm build` again
#   TAURI_CI_CONFIG=<path>   the overlay's path (relative to the repo root; default
#                            .cache/tauri-ci.json), one per build running side by side
set -euo pipefail
cd "$(dirname "$0")/../.."
source scripts/ci/channel.sh

: "${BUILD_VERSION:?run after version.sh (dotenv BUILD_VERSION)}"
code="${ANDROID_VERSION_CODE:-1}"
identifier="$app_id"
[[ "${1:-}" == --android ]] && identifier="$app_android_id"
mkdir -p .cache
path="$PWD/${TAURI_CI_CONFIG:-.cache/tauri-ci.json}"
mkdir -p "$(dirname "$path")"
build=""
[[ -n "${MWT_PREBUILT_FRONTEND:-}" ]] && build='"build":{"beforeBuildCommand":""},'
icons=""
for file in 32x32.png 128x128.png 128x128@2x.png icon.icns icon.ico icon.png; do
  icons+="${icons:+,}\"$app_icons/$file\""
done
printf '{"version":"%s","identifier":"%s","productName":"%s","mainBinaryName":"%s",%s"bundle":{"icon":[%s],"android":{"versionCode":%s}}}\n' \
  "$BUILD_VERSION" "$identifier" "$app_name" "$app_bin" "$build" "$icons" "$code" >"$path"
echo "$path"
