#!/usr/bin/env bash
# Job environment shared by every lane: sccache in a directory of the checkout
# (that is what the lane cache saves and restores), one absolute cargo target
# dir (tauri runs cargo from crates/app), and the dist/ that
# tauri::generate_context! wants to exist (the UI is not built for a compile check).
set -euo pipefail

mkdir -p "$GITHUB_WORKSPACE/.cache/sccache" "$GITHUB_WORKSPACE/dist"
{
  echo "SCCACHE_DIR=$GITHUB_WORKSPACE/.cache/sccache"
  echo "RUSTC_WRAPPER=sccache"
  echo "CARGO_TARGET_DIR=$GITHUB_WORKSPACE/.rust-target"
  echo "RUST_LOG=info"
} >>"$GITHUB_ENV"
