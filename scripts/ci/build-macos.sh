#!/usr/bin/env bash
# macOS universal (Apple Silicon + Intel) .dmg and a tarball of the .app, on a
# Mac: the build:macos job (GitLab macOS runner) or cargo xtask build
# (here or over ssh). Tauri cannot build for macOS from another OS.
#
#   MWT_APP_NAME=... [MWT_TAURI_CONFIG=<overlay>] scripts/ci/build-macos.sh <version> <out-dir>
#
# MWT_APP_NAME is the channel's product name (the .app folder); the overlay is written here by
# `cargo xtask tauri-config` when the caller gives none (the Mac has cargo after `mise install`).
#
# Signing: with APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD and
# APPLE_SIGNING_IDENTITY (+ APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID to
# notarize) tauri signs itself; without them the app is signed ad hoc and
# Gatekeeper asks to confirm the first start (docs/ci.md).
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
out="$2"
[[ "$(uname -s)" == Darwin ]] || { echo "build-macos.sh runs on macOS only" >&2; exit 1; }

export MISE_YES=1
if ! command -v mise >/dev/null; then
  curl -fsSL https://mise.run | sh
  export PATH="$HOME/.local/bin:$PATH"
fi
mise trust mise.toml
mise install node pnpm rust
eval "$(mise env -s bash)"
export CARGO_TARGET_DIR="$PWD/.rust-target"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
# In CI the store sits in the project, where the job cache keeps it.
if [[ -n "${CI:-}" ]]; then
  pnpm config set store-dir "$PWD/.cache/pnpm" >/dev/null
fi
pnpm install --frozen-lockfile --prefer-offline

if [[ -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "WARNING: no APPLE_SIGNING_IDENTITY - ad-hoc signature, not notarized" >&2
  export APPLE_SIGNING_IDENTITY="-"
fi
app_name="${MWT_APP_NAME:?the channel product name}"
if [[ -z "${MWT_TAURI_CONFIG:-}" ]]; then
  MWT_TAURI_CONFIG="$(cargo xtask tauri-config --channel "${RELEASE_CHANNEL:-dev}" --version "$version")"
fi
config="$MWT_TAURI_CONFIG"
# Bundles of an earlier build (the other channel names them differently)
# would be collected as well.
rm -rf "$CARGO_TARGET_DIR/universal-apple-darwin/release/bundle"
pnpm tauri build --ci --target universal-apple-darwin --bundles app,dmg --config "$config"

bundle="$CARGO_TARGET_DIR/universal-apple-darwin/release/bundle"
mkdir -p "$out"
dmgs=("$bundle"/dmg/*.dmg)
[[ ${#dmgs[@]} -eq 1 && -f "${dmgs[0]}" ]] || { echo "expected one .dmg in $bundle/dmg" >&2; exit 1; }
cp "${dmgs[0]}" "$out/multi-wallet-tracker-$version-macos-universal.dmg"
tar -C "$bundle/macos" -czf "$out/multi-wallet-tracker-$version-macos-universal.app.tar.gz" "$app_name.app"
ls -l "$out"
