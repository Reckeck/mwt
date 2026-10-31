#!/usr/bin/env bash
# Android APK + AAB for ANDROID_TARGETS (default: all three ABIs), signed by
# sign-android.sh. Runs in build:android after setup-linux.sh and
# setup-android.sh, and in the ci-android image for a local build.
#
#   MWT_TAURI_CONFIG=<overlay> MWT_APP_ICONS=icons/dev scripts/ci/build-android.sh <version> <out-dir>
#
# MWT_TAURI_CONFIG is the Android overlay (`cargo xtask tauri-config --android`), MWT_APP_ICONS
# the channel's icon folder under crates/app (`Channel::env` in crates/xtask).
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
out="$2"
# `tauri icon` writes the Android icons into the generated project, and
# rewrites (and adds to) the committed desktop icons on the way: put the
# folder back as it was. (cargo xtask build mounts a private copy of the
# folder and sets MWT_ICONS_ISOLATED: other builds read the real one meanwhile.)
if [[ -z "${MWT_ICONS_ISOLATED:-}" ]]; then
  icons="$(mktemp -d)" && chmod 755 "$icons"
  cp -a crates/app/icons/. "$icons/"
  trap 'rm -rf crates/app/icons && mv "$icons" crates/app/icons' EXIT
fi

config="${MWT_TAURI_CONFIG:?the Android Tauri config overlay (cargo xtask tauri-config --android)}"
app_icons="${MWT_APP_ICONS:?the channel icon folder, e.g. icons/dev}"
# The Gradle project is generated, not committed (crates/app/gen/android).
# Its Java package is the channel's identifier: a project left by the other
# channel would build both, so it starts over.
rm -rf crates/app/gen/android
pnpm tauri android init --ci --config "$config"
# The adaptive icon of the channel: background, foreground, monochrome.
pnpm tauri icon "crates/app/$app_icons/android/manifest.json"
# shellcheck disable=SC2086  # a space-separated list of ABIs
pnpm tauri android build --ci --apk --aab --target ${ANDROID_TARGETS:-aarch64 armv7 x86_64} --config "$config"
scripts/ci/sign-android.sh "$version" "$out"
