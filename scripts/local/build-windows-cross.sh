#!/usr/bin/env bash
# Windows NSIS installer and portable zip, cross-built from Linux with
# cargo-xwin, inside the image of windows-cross.Dockerfile (after
# setup-linux.sh). The MSI needs WiX, which runs on Windows only: that one
# comes from CI (build-windows.ps1). Names match build-windows.ps1.
#
#   MWT_TAURI_CONFIG=<overlay> MWT_APP_BIN=... scripts/local/build-windows-cross.sh <version> <out-dir>
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
out="$2"
target=x86_64-pc-windows-msvc
config="${MWT_TAURI_CONFIG:?the Tauri config overlay (cargo xtask tauri-config)}"
app_bin="${MWT_APP_BIN:?the channel binary name}"
# Bundles of an earlier build (the other channel names them differently)
# would be collected as well.
rm -rf "${CARGO_TARGET_DIR:-.rust-target}/$target/release/bundle"
pnpm tauri build --ci --runner cargo-xwin --target "$target" --bundles nsis --config "$config"

release_dir="${CARGO_TARGET_DIR:-.rust-target}/$target/release"
setups=("$release_dir"/bundle/nsis/*-setup.exe)
[[ ${#setups[@]} -eq 1 && -f "${setups[0]}" ]] || { echo "expected one NSIS installer in $release_dir/bundle/nsis" >&2; exit 1; }
mkdir -p "$out"
out="$(realpath "$out")"
cp "${setups[0]}" "$out/multi-wallet-tracker-$version-x64-setup.exe"
portable="$(mktemp -d)"
cp "$release_dir/$app_bin.exe" "$portable/"
(cd "$portable" && zip -q "$out/multi-wallet-tracker-$version-windows-x64-portable.zip" "$app_bin.exe")
rm -rf "$portable"
ls -l "$out"/*windows* "$out"/*setup.exe
