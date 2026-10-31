#!/usr/bin/env bash
# Build and push the CI images (node, linux, android) that are not in ghcr yet
# under the tag scripts/ci/image-tag.sh prints. Writes `tag` and `base`
# (lower-case ghcr.io/<owner>/<repo>) to $GITHUB_OUTPUT. FORCE=1 rebuilds.
#
# Needs a docker login to ghcr.io and `packages: write`.
set -euo pipefail
cd "$(dirname "$0")/../.."

tag="$(scripts/ci/image-tag.sh)"
base="ghcr.io/$(printf '%s' "$GITHUB_REPOSITORY" | tr '[:upper:]' '[:lower:]')"
{
  echo "tag=$tag"
  echo "base=$base"
} >>"$GITHUB_OUTPUT"

missing=()
for target in node linux android; do
  if [[ -z "${FORCE:-}" ]] && docker manifest inspect "$base/ci-$target:$tag" >/dev/null 2>&1; then
    echo "$base/ci-$target:$tag is already pushed"
  else
    missing+=("$target")
  fi
done
if [[ ${#missing[@]} -eq 0 ]]; then
  exit 0
fi

# The android image is ~8 GB on top of what the runner image holds.
sudo rm -rf /usr/share/dotnet /opt/ghc /usr/local/lib/android /opt/hostedtoolcache/CodeQL || true
df -h / | tail -1

for target in "${missing[@]}"; do
  image="$base/ci-$target:$tag"
  # The earlier stage is in the local layer cache: linux reuses node, android reuses linux.
  docker build --target "$target" -f scripts/ci/ci.Dockerfile \
    --label "org.opencontainers.image.source=https://github.com/$GITHUB_REPOSITORY" \
    -t "$image" .
  docker push "$image"
done
