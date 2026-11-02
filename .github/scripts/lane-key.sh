#!/usr/bin/env bash
# Print the cache key of one lane: <lane>-<hash>. The hash is everything that
# changes how the DEPENDENCIES compile and nothing about our own code:
#   - the rustc version (rust from mise.toml, baked in the image or installed by mise),
#   - the sccache version (entries of another version never hit),
#     a build-* lane has no rustc on the runner: the image tag (Dockerfile + mise.toml [tools])
#     and SCCACHE_PIN (the sccache the Windows job installs),
#   - the channel (RELEASE_CHANNEL: dev builds without LTO, prod with the full one; xtask sets the
#     switches itself, so the key must name the channel) and any release profile switch set in
#     the workflow env,
#   - the CI image tag (CI_IMAGE_TAG, empty off-image),
#   - the dev profile switches set in the workflow env,
#   - the external packages of Cargo.lock (workspace crates, whose version a release
#     bumps, are left out),
#   - .cargo/config.toml and deps-cook.sh,
#   - every Cargo.toml without its `version =` line (features, [profile.*], dependency lists).
# Node does not affect compiled crates, pnpm has its own cache (pnpm-lock.yaml).
#
#   .github/scripts/lane-key.sh check-linux
#   .github/scripts/lane-key.sh build-android-dev
set -euo pipefail
cd "$(dirname "$0")/../.."
lane="${1:?lane name}"

digest() {
  if command -v sha256sum >/dev/null; then sha256sum; else shasum -a 256; fi
}

{
  if [[ "$lane" == build-* ]]; then
    scripts/ci/image-tag.sh
    echo "sccache=${SCCACHE_PIN:-}"
  else
    rustc -V
    sccache --version
  fi
  echo "image=${CI_IMAGE_TAG:-}"
  echo "debug=${CARGO_PROFILE_DEV_DEBUG:-}"
  echo "channel=${RELEASE_CHANNEL:-}"
  echo "lto=${CARGO_PROFILE_RELEASE_LTO:-} units=${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-}"
  cat .cargo/config.toml .github/scripts/deps-cook.sh
  awk -v RS= '/(^|\n)source = /' Cargo.lock
  LC_ALL=C find . -name Cargo.toml -not -path '*/node_modules/*' -not -path './.rust-target/*' \
    -not -path './crates/app/gen/*' | LC_ALL=C sort | while read -r manifest; do
    echo "# $manifest"
    grep -v '^version = ' "$manifest" || true
  done
} | digest | cut -c1-16 | sed "s/^/${lane}-/"
