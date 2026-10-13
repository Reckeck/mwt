#!/usr/bin/env bash
# Upload everything in <release-dir> to the Generic Package Registry and
# create the GitLab Release v<version> on this commit (the API creates the tag;
# tag pipelines do not run — see workflow:rules).
#
#   scripts/ci/release.sh [--pre] <version> <release-dir>
#
#   (default)  a release from main: «Multi Wallet Tracker X.Y.Z», notes from
#              the CHANGELOG section of the version
#   --pre      a pre-release from development: «Pre-release X.Y.Z-N», notes
#              = the commits since the last release (git log; full clone)
#
# Authenticates with CI_JOB_TOKEN; set RELEASE_TOKEN (a project access token
# with `api`) if the job token is not allowed to create releases here.
set -euo pipefail

pre=""
if [[ "${1:-}" == --pre ]]; then
  pre=1
  shift
fi
version="$1"
cd "$2"
tag="v$version"

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

section() { # the CHANGELOG section headed «## [$1]»
  awk -v ver="$1" '
    $0 ~ "^## \\[" ver "\\]" {on=1; next}
    on && /^## \[/ {exit}
    on {print}' "$CI_PROJECT_DIR/CHANGELOG.md"
}
commit_link="[\`${CI_COMMIT_SHORT_SHA}\`]($CI_PROJECT_URL/-/commit/$CI_COMMIT_SHA)"
if [[ -n "$pre" ]]; then
  name="Pre-release $version"
  # Every commit since the last release, newest first (release tags only).
  since="$(git describe --tags --abbrev=0 --match 'v[0-9]*' --exclude 'v*-*' 2>/dev/null || true)"
  commits="$(git log -n 60 --no-merges --format="- %s ([\`%h\`]($CI_PROJECT_URL/-/commit/%H))" \
    ${since:+"$since"..}HEAD)"
  notes="**Пре-релиз** из \`${CI_COMMIT_BRANCH:-development}\` (коммит $commit_link) — для проверки, не для работы с настоящими данными.

### Изменения с ${since:-начала проекта}

${commits:-_нет коммитов_}"
else
  name="Multi Wallet Tracker $version"
  notes="$(section "$version")"
  [[ -n "$notes" ]] || notes="Изменения — в [CHANGELOG.md]($CI_PROJECT_URL/-/blob/$tag/CHANGELOG.md)."
fi

payload="$(jq -n --arg tag "$tag" --arg ref "$CI_COMMIT_SHA" --arg name "$name" --arg notes "$notes" \
  --argjson links "$links" '{tag_name: $tag, ref: $ref, name: $name, description: $notes, assets: {links: $links}}')"
curl -fsS "${auth[@]}" --header 'Content-Type: application/json' \
  --data "$payload" "$api/releases" >/dev/null
echo "$name: $CI_PROJECT_URL/-/releases/$tag"
