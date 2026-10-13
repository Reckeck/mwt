#!/usr/bin/env bash
# After a release from main, open the merge request main → development so the
# release (version bump, hotfixes) reaches the next pre-releases too. Does
# nothing when one is already open.
#
#   scripts/ci/backmerge.sh <version>
#
# Needs RELEASE_TOKEN (project access token, `api`); without it the job only
# reminds to merge by hand.
set -euo pipefail

version="$1"
source_branch="${PROD_BRANCH:-main}"
target_branch="${DEV_BRANCH:-development}"
if [[ -z "${RELEASE_TOKEN:-}" ]]; then
  echo "WARNING: RELEASE_TOKEN is not set — open the MR $source_branch → $target_branch by hand" >&2
  exit 0
fi
auth=(--header "PRIVATE-TOKEN: $RELEASE_TOKEN")
api="$CI_API_V4_URL/projects/$CI_PROJECT_ID"

open="$(curl -fsS "${auth[@]}" \
  "$api/merge_requests?state=opened&source_branch=$source_branch&target_branch=$target_branch" | jq 'length')"
if ((open > 0)); then
  echo "an MR $source_branch → $target_branch is already open"
  exit 0
fi
payload="$(jq -n --arg source "$source_branch" --arg target "$target_branch" --arg version "$version" \
  '{source_branch: $source, target_branch: $target, remove_source_branch: false,
    title: ("Back-merge " + $source + " into " + $target + " after v" + $version),
    description: ("Release v" + $version + " is out. Merge it back so " + $target + " carries it, then raise the version on " + $target + " to the next one.")}')"
if url="$(curl -fsS "${auth[@]}" --header 'Content-Type: application/json' --data "$payload" \
  "$api/merge_requests" | jq -r '.web_url')"; then
  echo "opened $url"
else
  echo "could not open the MR (nothing to merge back?) — check $source_branch → $target_branch by hand" >&2
fi
