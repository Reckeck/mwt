#!/usr/bin/env bash
# Android APK + AAB for ANDROID_TARGETS (default: all three ABIs), signed by
# sign-android.sh. Runs in build:android after setup-linux.sh and
# setup-android.sh, and in the ci-android image for a local build.
#
#   scripts/ci/build-android.sh <version> <out-dir>
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
out="$2"
# `tauri icon` writes the Android icons into the generated project, and
# rewrites (and adds to) the committed desktop icons on the way: put the
# folder back as it was.
icons="$(mktemp -d)" && chmod 755 "$icons"
cp -a crates/app/icons/. "$icons/"
trap 'rm -rf crates/app/icons && mv "$icons" crates/app/icons' EXIT

# The Gradle project is generated, not committed (crates/app/gen/android).
pnpm tauri android init --ci
pnpm tauri icon crates/app/icons/icon.png
config="$(BUILD_VERSION="$version" scripts/ci/tauri-config.sh)"
# shellcheck disable=SC2086  # a space-separated list of ABIs
pnpm tauri android build --ci --apk --aab --target ${ANDROID_TARGETS:-aarch64 armv7 x86_64} --config "$config"
scripts/ci/sign-android.sh "$version" "$out"
