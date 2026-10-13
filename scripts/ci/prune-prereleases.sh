#!/usr/bin/env bash
# Keep only the newest pre-releases: delete older ones — the GitLab Release,
# its tag and the generic package version (development pushes often).
#
#   scripts/ci/prune-prereleases.sh [keep]     # default PRERELEASES_KEPT or 10
#
# Needs RELEASE_TOKEN (project access token, `api`, Maintainer): the job token
# may not delete. Without it the job only warns.
set -euo pipefail

keep="${1:-${PRERELEASES_KEPT:-10}}"
if [[ -z "${RELEASE_TOKEN:-}" ]]; then
  echo "WARNING: RELEASE_TOKEN is not set — old pre-releases stay (set it to keep only $keep)" >&2
  exit 0
fi
auth=(--header "PRIVATE-TOKEN: $RELEASE_TOKEN")
api="$CI_API_V4_URL/projects/$CI_PROJECT_ID"
urlencode() { jq -rn --arg value "$1" '$value | @uri'; }

# Pre-release tags are vX.Y.Z-N; releases come newest first.
mapfile -t old < <(
  curl -fsS "${auth[@]}" "$api/releases?per_page=100&order_by=released_at&sort=desc" |
    jq -r '.[].tag_name | select(test("^v[0-9]+\\.[0-9]+\\.[0-9]+-[0-9]+$"))' |
    tail -n "+$((keep + 1))"
)
if ((${#old[@]} == 0)); then
  echo "at most $keep pre-releases: nothing to prune"
  exit 0
fi

packages="$(curl -fsS "${auth[@]}" \
  "$api/packages?package_type=generic&package_name=multi-wallet-tracker&per_page=100")"
for tag in "${old[@]}"; do
  echo "pruning $tag"
  encoded="$(urlencode "$tag")"
  curl -fsS -X DELETE "${auth[@]}" "$api/releases/$encoded" >/dev/null || echo "  release: not deleted"
  curl -fsS -X DELETE "${auth[@]}" "$api/repository/tags/$encoded" >/dev/null || echo "  tag: not deleted"
  for id in $(jq -r --arg version "${tag#v}" '.[] | select(.version == $version) | .id' <<<"$packages"); do
    curl -fsS -X DELETE "${auth[@]}" "$api/packages/$id" >/dev/null || echo "  package $id: not deleted"
  done
done
