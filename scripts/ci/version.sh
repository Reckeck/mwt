#!/usr/bin/env bash
# Print the app version (crates/app/tauri.conf.json). With a tag in
# CI_COMMIT_TAG, also fail unless the tag is v<version> and Cargo.toml agrees.
set -euo pipefail

version="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' crates/app/tauri.conf.json)"
cargo_version="$(sed -n '/^\[workspace.package\]/,/^\[/ s/^version = "\(.*\)"$/\1/p' Cargo.toml)"
[[ -n "$version" ]] || { echo "no version in crates/app/tauri.conf.json" >&2; exit 1; }

if [[ -n "${CI_COMMIT_TAG:-}" ]]; then
  if [[ "$CI_COMMIT_TAG" != "v$version" ]]; then
    echo "tag $CI_COMMIT_TAG does not match tauri.conf.json version $version (expected v$version)" >&2
    exit 1
  fi
  if [[ "$cargo_version" != "$version" ]]; then
    echo "Cargo.toml workspace version $cargo_version differs from tauri.conf.json $version" >&2
    exit 1
  fi
fi
echo "$version"
