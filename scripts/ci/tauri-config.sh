#!/usr/bin/env bash
# Write the Tauri config overlay of a CI build — the version from version.sh
# (BUILD_VERSION), the Android versionCode and the channel's identifier, name
# and binary (channel.sh) — and print its path, for
# `tauri build --config <path>`. A file, not inline JSON: it survives every
# shell's quoting (build-windows.ps1 writes the same file itself).
#
#   scripts/ci/tauri-config.sh [--android]   # --android: the Android identifier
set -euo pipefail
cd "$(dirname "$0")/../.."
source scripts/ci/channel.sh

: "${BUILD_VERSION:?run after version.sh (dotenv BUILD_VERSION)}"
code="${ANDROID_VERSION_CODE:-1}"
identifier="$app_id"
[[ "${1:-}" == --android ]] && identifier="$app_android_id"
mkdir -p .cache
path="$PWD/.cache/tauri-ci.json"
printf '{"version":"%s","identifier":"%s","productName":"%s","mainBinaryName":"%s","bundle":{"android":{"versionCode":%s}}}\n' \
  "$BUILD_VERSION" "$identifier" "$app_name" "$app_bin" "$code" >"$path"
echo "$path"
