#!/usr/bin/env bash
# Job environment shared by every lane: sccache in a directory of the checkout
# (that is what the lane cache saves and restores) and the dist/ that
# tauri::generate_context! wants to exist (the UI is not built for a compile check).
set -euo pipefail

mkdir -p "$GITHUB_WORKSPACE/.cache/sccache" "$GITHUB_WORKSPACE/dist"
{
  echo "SCCACHE_DIR=$GITHUB_WORKSPACE/.cache/sccache"
  echo "RUSTC_WRAPPER=sccache"
  echo "RUST_LOG=info"
} >>"$GITHUB_ENV"
