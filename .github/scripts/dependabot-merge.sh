#!/usr/bin/env bash
# Weekly merge of the Dependabot PRs into development (dependabot-merge.yml).
#
# Merged: an open Dependabot PR whose checks are all green (ci-ok among them), that merges
# cleanly, and that is not a major update:
#   - "Bump X from A to B ..." (one package, grouped or not): A -> B keeps the major version
#     (for 0.x the minor one, as Cargo and npm treat it);
#   - a group of several packages: only the minor/patch groups of .github/dependabot.yml.
# Left for review: majors, red or running checks, conflicts. A PR that conflicts after an
# earlier merge of this run is rebased by Dependabot and waits for next week.
# A merge by GITHUB_TOKEN starts no push workflows, so the dev pre-release is started here
# (release.yml, workflow_dispatch) once something is merged.
#
#   GH_TOKEN=... REPO=owner/name [DRY_RUN=1] .github/scripts/dependabot-merge.sh
set -euo pipefail

repo="${REPO:?REPO}"
dry="${DRY_RUN:-0}"
base=development
minor_groups=" cargo-minor npm-minor actions "

summary="${GITHUB_STEP_SUMMARY:-/dev/stdout}"
say() { echo "$*"; echo "- $*" >>"$summary"; }

# Exit 0 when going from $1 to $2 changes the major version (the minor one for 0.x).
is_major() {
  local from_major from_minor to_major to_minor
  IFS=. read -r from_major from_minor _ <<<"${1#v}"
  IFS=. read -r to_major to_minor _ <<<"${2#v}"
  if [[ "$from_major" != "$to_major" ]]; then return 0; fi
  [[ "$from_major" == 0 && "${from_minor:-0}" != "${to_minor:-0}" ]]
}

# Exit 0 when the title is an update this script may merge; prints why not otherwise.
allowed() {
  local title="$1"
  if [[ "$title" =~ from\ ([^ ]+)\ to\ ([^ ]+) ]]; then
    if is_major "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"; then
      echo "major update"
      return 1
    fi
    return 0
  fi
  if [[ "$title" =~ the\ ([A-Za-z0-9_-]+)\ group ]] && [[ "$minor_groups" == *" ${BASH_REMATCH[1]} "* ]]; then
    return 0
  fi
  echo "not a minor/patch group"
  return 1
}

# Exit 0 when every check is finished and green and ci-ok passed; prints why not otherwise.
green() {
  local json="$1"
  jq -er '
    .statusCheckRollup as $checks
    | ($checks | length) > 0
      and all($checks[]; (.conclusion // .state) as $result
                         | ["SUCCESS", "SKIPPED", "NEUTRAL"] | index($result))
      and any($checks[]; .name == "ci-ok" and .conclusion == "SUCCESS")
  ' <<<"$json" >/dev/null && return 0
  echo "checks are not all green"
  return 1
}

echo "## Dependabot PRs into $base" >>"$summary"
merged=0
prs="$(gh pr list --repo "$repo" --base "$base" --author 'app/dependabot' --state open \
  --json number --jq '.[].number' | sort -n)"
for number in $prs; do
  json="$(gh pr view "$number" --repo "$repo" --json title,mergeable,headRefOid,statusCheckRollup)"
  title="$(jq -r .title <<<"$json")"
  if ! why="$(allowed "$title")" || ! why="$(green "$json")"; then
    say "#$number skipped ($why): $title"
    continue
  fi
  if [[ "$(jq -r .mergeable <<<"$json")" != MERGEABLE ]]; then
    say "#$number skipped (does not merge cleanly now): $title"
    continue
  fi
  if [[ "$dry" == 1 ]]; then
    say "#$number would be merged: $title"
    continue
  fi
  if gh pr merge "$number" --repo "$repo" --squash --delete-branch \
    --match-head-commit "$(jq -r .headRefOid <<<"$json")"; then
    say "#$number merged: $title"
    merged=$((merged + 1))
  else
    say "#$number skipped (merge refused): $title"
  fi
done

if [[ "$merged" -gt 0 ]]; then
  gh workflow run release.yml --repo "$repo" --ref "$base" -f channel=dev -f publish=true
  say "release.yml started on $base for the dev pre-release ($merged merged)"
else
  say "nothing merged"
fi
