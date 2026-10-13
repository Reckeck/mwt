#!/usr/bin/env bash
# Write the Tauri config overlay of a CI build — the version from version.sh
# (BUILD_VERSION) and the Android versionCode — and print its path, for
# `tauri build --config <path>`. A file, not inline JSON: it survives every
# shell's quoting (build-windows.ps1 writes the same file itself).
set -euo pipefail
cd "$(dirname "$0")/../.."

: "${BUILD_VERSION:?run after version.sh (dotenv BUILD_VERSION)}"
code="${ANDROID_VERSION_CODE:-1}"
mkdir -p .cache
path="$PWD/.cache/tauri-ci.json"
printf '{"version":"%s","bundle":{"android":{"versionCode":%s}}}\n' "$BUILD_VERSION" "$code" >"$path"
echo "$path"
