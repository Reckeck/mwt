#!/usr/bin/env bash
# The PR title is a Conventional Commits subject (pr-title.yml). A squash merge makes the title the
# commit subject on development/main, and git-cliff (cliff.toml, filter_unconventional) drops every
# other subject from CHANGELOG.md and from the version bump.
#
#   <type>(<scope>)!: <summary>      type: feat fix perf refactor docs test chore ci build
#   at most 72 characters (AGENTS.md, "Commit"); Dependabot titles are exempt from the length
#
#   TITLE="..." [AUTHOR=login] .github/scripts/check-pr-title.sh
#   .github/scripts/check-pr-title.sh --self-test
set -euo pipefail

max_length=72
pattern='^(feat|fix|perf|refactor|docs|test|chore|ci|build)(\([a-z0-9][a-z0-9._/-]*\))?!?: [^ ].*$'

# Prints why the title is refused; prints nothing for a good one.
problem() {
  local title="$1" author="$2"
  if [[ ! "$title" =~ $pattern ]]; then
    echo "not a Conventional Commits subject: expected '<type>(<scope>): <summary>', type one of feat fix perf refactor docs test chore ci build"
  elif [[ "$author" != "dependabot[bot]" && ${#title} -gt $max_length ]]; then
    echo "${#title} characters, at most $max_length"
  fi
}

self_test() {
  local failed=0 expected title author got
  while IFS='|' read -r expected author title; do
    got=ok
    [[ -n "$(problem "$title" "$author")" ]] && got=bad
    if [[ "$got" != "$expected" ]]; then
      echo "self-test: expected $expected, got $got: '$title' ($author)"
      failed=1
    fi
  done <<'CASES'
ok||refactor(core): pin the crypto gate list (core-strangler#T39)
ok||feat: import operations from the own CSV export
ok||fix(sync)!: refuse protocol 4 peers
ok||chore(release): v0.3.0
ok||chore(release): back-merge v0.3.0 into development
ok|dependabot[bot]|fix(deps): Bump the cargo-minor group across 1 directory with 5 updates and more words
bad||Core strangler C6: pin the crypto gate list to the record kinds (T39)
bad||Vault reset wave 2: drop settings.crypto
bad||feature(core): not a type
bad||refactor(Core): scope is lower case
bad||refactor(core):missing space
bad||refactor(core): move notifications, reminders, agenda, automations views out of the vault
bad|someone|fix(deps): Bump the cargo-minor group across 1 directory with 5 updates and more words
CASES
  return "$failed"
}

if [[ "${1:-}" == "--self-test" ]]; then
  self_test && echo "self-test: ok"
  exit
fi

title="${TITLE:?TITLE}"
reason="$(problem "$title" "${AUTHOR:-}")"
if [[ -n "$reason" ]]; then
  echo "::error title=PR title::$reason. Title: $title"
  echo "Rename the PR, e.g. 'refactor(core): move reports out of the vault (core-strangler#T18)'."
  exit 1
fi
echo "PR title ok: $title"
