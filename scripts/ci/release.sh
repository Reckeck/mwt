#!/usr/bin/env bash
# Upload everything in <release-dir> to the Generic Package Registry and
# create the GitLab Release for CI_COMMIT_TAG with links to the files.
#
#   scripts/ci/release.sh <version> <release-dir>
#
# Authenticates with CI_JOB_TOKEN; set RELEASE_TOKEN (a project access token
# with `api`) if the job token is not allowed to create releases here.
set -euo pipefail

version="$1"
cd "$2"

sha256sum -- * | grep -v ' SHA256SUMS$' >SHA256SUMS
cat SHA256SUMS

if [[ -n "${RELEASE_TOKEN:-}" ]]; then
  auth=(--header "PRIVATE-TOKEN: $RELEASE_TOKEN")
else
  auth=(--header "JOB-TOKEN: $CI_JOB_TOKEN")
fi
api="$CI_API_V4_URL/projects/$CI_PROJECT_ID"
base="$api/packages/generic/multi-wallet-tracker/$version"

links='[]'
for file in *; do
  curl -fsS "${auth[@]}" --upload-file "$file" "$base/$file" >/dev/null
  case "$file" in
    SHA256SUMS) kind=other ;;
    *) kind=package ;;
  esac
  links="$(jq --arg name "$file" --arg url "$base/$file" --arg kind "$kind" \
    '. + [{name: $name, url: $url, link_type: $kind, direct_asset_path: ("/" + $name)}]' <<<"$links")"
done

# Notes: the CHANGELOG section of this version, else a pointer to the file.
notes="$(awk -v ver="$version" '
  $0 ~ "^## \\[" ver "\\]" {on=1; next}
  on && /^## \[/ {exit}
  on {print}' "$CI_PROJECT_DIR/CHANGELOG.md")"
[[ -n "$notes" ]] || notes="Изменения — в [CHANGELOG.md]($CI_PROJECT_URL/-/blob/$CI_COMMIT_TAG/CHANGELOG.md)."

payload="$(jq -n --arg tag "$CI_COMMIT_TAG" --arg name "Multi Wallet Tracker $version" --arg notes "$notes" \
  --argjson links "$links" '{tag_name: $tag, name: $name, description: $notes, assets: {links: $links}}')"
curl -fsS "${auth[@]}" --header 'Content-Type: application/json' \
  --data "$payload" "$api/releases" >/dev/null
echo "release $CI_COMMIT_TAG created: $CI_PROJECT_URL/-/releases/$CI_COMMIT_TAG"
