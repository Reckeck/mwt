#!/usr/bin/env bash
# Compile only the dependencies of a lane, so sccache holds them (the job
# `deps`). cargo-chef rebuilds a skeleton of the workspace (our crates are empty
# stubs, manifests and Cargo.lock are real), so the feature set of every
# dependency equals the real build's. It runs in a scratch copy: `cook` writes
# its stubs into the tree it runs in.
#
# Each lane cooks what that lane's job compiles:
#   check-linux     cargo xtask check: cargo test/run (dev build) and clippy, whole workspace
#   clippy-android  xtask (dev build, host) + clippy -p trade-app for aarch64-linux-android
#   clippy-macos    xtask (dev build, host) + clippy -p trade-app
#   clippy-windows  clippy -p trade-app (no xtask there, see ci.yml)
#
#   .github/scripts/deps-cook.sh <lane>
set -euo pipefail
cd "$(dirname "$0")/../.."
lane="${1:?lane name}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git archive HEAD | tar -x -C "$work"
cd "$work"
export CARGO_TARGET_DIR="$work/.deps-target"
cargo chef prepare --recipe-path recipe.json

cook() {
  echo "::group::cargo chef cook $*"
  cargo chef cook --recipe-path recipe.json "$@"
  echo "::endgroup::"
}

case "$lane" in
  check-linux)
    cook --workspace --all-targets
    cook --clippy --workspace --all-targets
    ;;
  clippy-android)
    # The NDK's clang for C dependencies built by build scripts (as xtask does).
    ndk="${ANDROID_NDK_HOME:-${NDK_HOME:?no Android NDK}}"
    bin="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
    export CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang"
    export AR_aarch64_linux_android="$bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
    cook -p xtask
    cook --clippy -p trade-app --target aarch64-linux-android
    ;;
  clippy-macos)
    cook -p xtask
    cook --clippy -p trade-app
    ;;
  clippy-windows)
    cook --clippy -p trade-app
    ;;
  *)
    echo "unknown lane: $lane" >&2
    exit 2
    ;;
esac
