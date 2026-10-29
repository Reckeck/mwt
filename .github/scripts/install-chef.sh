#!/usr/bin/env bash
# cargo-chef for `deps`. Linux: the release binary (checked against its published
# sha256); Windows and macOS have no release binary, so it is built from source.
# Only the job `deps` needs it, and only when the lane has no cache yet.
set -euo pipefail
version="0.1.78"

if [[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]]; then
  name="cargo-chef-x86_64-unknown-linux-musl"
  base="https://github.com/LukeMathWalker/cargo-chef/releases/download/v$version"
  dir="$(mktemp -d)"
  curl -fsSL -o "$dir/$name.tar.xz" "$base/$name.tar.xz"
  curl -fsSL -o "$dir/$name.sha256" "$base/$name.tar.xz.sha256"
  expected="$(cut -d' ' -f1 "$dir/$name.sha256")"
  actual="$(sha256sum "$dir/$name.tar.xz" | cut -d' ' -f1)"
  if [[ "$expected" != "$actual" ]]; then
    echo "cargo-chef checksum mismatch: $expected != $actual" >&2
    exit 1
  fi
  tar -xJ -C "$dir" -f "$dir/$name.tar.xz"
  install -m 0755 "$dir/$name/cargo-chef" /usr/local/bin/cargo-chef
else
  cargo install --locked --version "$version" cargo-chef
fi
cargo chef --version
