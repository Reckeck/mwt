#!/usr/bin/env bash
# Print the CI image tag: a hash of everything baked into the images — the
# Dockerfile, the apt package lists and the [tools] of mise.toml (tasks and
# env do not matter). CI_IMAGE_TAG in .gitlab-ci.yml must equal it; the
# ci:image job refuses to build otherwise.
#
#   scripts/ci/image-tag.sh            # → e.g. 3f9c2a1b7d4e
set -euo pipefail
cd "$(dirname "$0")/../.."

tools="$(awk '/^\[tools\]/{on=1; next} /^\[/{on=0} on && NF' mise.toml)"
{
  cat scripts/ci/ci.Dockerfile scripts/ci/apt-packages.sh
  printf '%s\n' "$tools"
} | sha256sum | cut -c1-12
