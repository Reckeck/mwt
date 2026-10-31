#!/usr/bin/env bash
# Job environment shared by every lane: sccache in a directory of the checkout
# (that is what the lane cache saves and restores) and the dist/ that
# tauri::generate_context! wants to exist (the UI is not built for a compile check).
set -euo pipefail

mkdir -p "$GITHUB_WORKSPACE/.cache/sccache" "$GITHUB_WORKSPACE/dist"
{
  echo "SCCACHE_DIR=$GITHUB_WORKSPACE/.cache/sccache"
  echo "RUSTC_WRAPPER=sccache"
  # For C/C++ only: sccache does not apply it to rustc, see the target dir in deps-cook.sh.
  echo "SCCACHE_BASEDIRS=$GITHUB_WORKSPACE"
  echo "RUST_LOG=info"
} >>"$GITHUB_ENV"
