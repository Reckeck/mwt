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

# The real job's target dir, to the byte: crates with a build script have OUT_DIR, and with it
# this path, in their sccache key. On Windows cargo sees a native path.
target="$PWD/.rust-target"
if command -v cygpath >/dev/null; then target="$(cygpath -w "$target")"; fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# In a job container the checkout belongs to another user.
git config --global --add safe.directory "$PWD"
git archive HEAD | tar -x -C "$work"
cd "$work"
# A flag, not CARGO_TARGET_DIR: sccache hashes CARGO_* variables, so the variable would make
# every key differ from the real build. chef needs it: it does not read .cargo/config.toml.
unset CARGO_TARGET_DIR
cargo chef prepare --recipe-path recipe.json

# cook <target dir> <cargo arguments>: xtask lanes are checks.rs: lane().
cook() {
  local dir="$1"
  shift
  echo "::group::cargo chef cook $*"
  cargo chef cook --recipe-path recipe.json --target-dir "$dir" "$@"
  echo "::endgroup::"
}

case "$lane" in
  check-linux)
    cook "$target" --workspace --all-targets
    cook "$target/lanes/clippy" --clippy --workspace --all-targets
    ;;
  clippy-android)
    # The NDK's clang for C dependencies built by build scripts (as xtask does).
    ndk="${ANDROID_NDK_HOME:-${NDK_HOME:?no Android NDK}}"
    bin="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
    export CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang"
    export AR_aarch64_linux_android="$bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
    cook "$target" -p xtask
    cook "$target/lanes/android" --clippy -p trade-app --target aarch64-linux-android
    ;;
  clippy-macos)
    cook "$target" -p xtask
    cook "$target/lanes/macos" --clippy -p trade-app
    ;;
  clippy-windows)
    cook "$target" --clippy -p trade-app
    ;;
  *)
    echo "unknown lane: $lane" >&2
    exit 2
    ;;
esac
