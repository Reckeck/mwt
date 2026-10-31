#!/usr/bin/env bash
# Delete the older caches of this ref and lane (decision: one cache per lane and
# branch), keeping the key just saved. Needs `actions: write` for GITHUB_TOKEN.
# API: https://docs.github.com/en/rest/actions/cache
#
#   GITHUB_TOKEN=... .github/scripts/deps-prune.sh <lane> <key-to-keep>
set -euo pipefail
lane="${1:?lane name}"
keep="${2:?key to keep}"
api="https://api.github.com/repos/$GITHUB_REPOSITORY/actions/caches"
auth=(-H "Authorization: Bearer $GITHUB_TOKEN" -H "X-GitHub-Api-Version: 2022-11-28" -H "Accept: application/vnd.github+json")

# `key` filters by prefix, `ref` by the ref the cache was saved under (refs/pull/N/merge for a PR).
ids="$(curl -fsS "${auth[@]}" -G "$api" --data-urlencode "ref=$GITHUB_REF" --data-urlencode "key=${lane}-" \
  --data-urlencode "per_page=100" | jq -r --arg keep "$keep" '.actions_caches[] | select(.key != $keep) | .id')"
for id in $ids; do
  echo "deleting the old cache $id of $lane on $GITHUB_REF"
  curl -fsS "${auth[@]}" -X DELETE "$api/$id" >/dev/null
done
