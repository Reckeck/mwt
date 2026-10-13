#!/usr/bin/env bash
# Build and push the CI images (node, linux, android) for CI_IMAGE_TAG, in
# the ci:image job (docker:dind). Skips a target whose tag is already in the
# registry, so re-running costs a few seconds. FORCE_CI_IMAGE=1 rebuilds.
set -euo pipefail

expected="$(scripts/ci/image-tag.sh)"
if [[ "$CI_IMAGE_TAG" != "$expected" ]]; then
  echo "CI_IMAGE_TAG in .gitlab-ci.yml is '$CI_IMAGE_TAG', but the image inputs hash to '$expected'." >&2
  echo "Set  CI_IMAGE_TAG: \"$expected\"  in .gitlab-ci.yml (scripts/ci/image-tag.sh prints it)." >&2
  exit 1
fi

echo "$CI_REGISTRY_PASSWORD" | docker login -u "$CI_REGISTRY_USER" --password-stdin "$CI_REGISTRY"
for target in node linux android; do
  image="$CI_REGISTRY_IMAGE/ci-$target:$CI_IMAGE_TAG"
  if [[ -z "${FORCE_CI_IMAGE:-}" ]] && docker manifest inspect "$image" >/dev/null 2>&1; then
    echo "$image already pushed"
    continue
  fi
  # The previous stage is in the local cache: linux reuses node, android reuses linux.
  docker build --target "$target" -f scripts/ci/ci.Dockerfile -t "$image" .
  docker push "$image"
done
