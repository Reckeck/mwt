#!/usr/bin/env bash
# Print the CI image tag: a hash of everything baked into the images — the
# Dockerfile, the apt package lists and the [tools] of mise.toml (tasks and
# env do not matter). images.yml builds and pushes the images under it
# (cargo xtask image-tag prints the same value).
#
#   scripts/ci/image-tag.sh            # → e.g. 3f9c2a1b7d4e
set -euo pipefail
cd "$(dirname "$0")/../.."

tools="$(awk '/^\[tools\]/{on=1; next} /^\[/{on=0} on && NF' mise.toml)"
{
  cat scripts/ci/ci.Dockerfile scripts/ci/apt-packages.sh scripts/ci/install-chrome.sh
  printf '%s\n' "$tools"
} | sha256sum | cut -c1-12
