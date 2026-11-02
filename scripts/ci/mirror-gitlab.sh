#!/usr/bin/env bash
# Pushes development, main and the tags of this checkout to the GitLab mirror
# (Reckeck/trade-app). GitLab.com Free has no pull mirroring, so GitHub pushes.
#
# Needs a full clone (fetch-depth 0) and GITLAB_MIRROR_KEY: the private half of
# an SSH deploy key with write access on the GitLab project (docs/ci.md, "Secrets").
# The mirror follows GitHub: a diverged branch on GitLab is overwritten.
set -euo pipefail

: "${GITLAB_MIRROR_KEY:?GITLAB_MIRROR_KEY is not set (docs/ci.md, Secrets)}"
url="${GITLAB_MIRROR_URL:-git@gitlab.com:Reckeck/trade-app.git}"

dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT
printf '%s\n' "$GITLAB_MIRROR_KEY" >"$dir/key"
chmod 600 "$dir/key"
# gitlab.com host keys (https://docs.gitlab.com/user/gitlab_com/#ssh-known_hosts-entries)
cat >"$dir/known_hosts" <<'KEYS'
gitlab.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAfuCHKVTjquxvt6CM6tdG4SLp1Btn/nOeHHE5UOzRdf
gitlab.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQCsj2bNKTBSpIYDEGk9KxsGh3mySTRgMtXL583qmBpzeQ+jqCMRgBqB98u3z++J1sKlXHWfM9dyhSevkMwSbhoR8XIq/U0tCNyokEi/ueaBMCvbcTHhO7FcwzY92WK4Yt0aGROY5qX2UKSeOvuP4D6TPqKF1onrSzH9bx9XUf2lEdWT/ia1NEKjunUqu1xOB/StKDHMoX4/OKyIzuS0q/T1zOATthvasJFoPrAjkohTyaDUz2LN5JoH839hViyEG82yB+MjcFV5MU3N1l1QL3cVUCh93xSaua1N85qivl+siMkPGbO5xR/En4iEY6K2XPASUEMaieWVNTRCtJ4S8H+9
gitlab.com ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBFSMqzJeV9rUzU4kWitGjeR4PWSa29SPqJ1fVkhtj3Hw9xjLVXVYrU9QlYWrOLXBpQ6KWjbjTDTdDkoohFzgbEY=
KEYS
export GIT_SSH_COMMAND="ssh -i $dir/key -o IdentitiesOnly=yes -o UserKnownHostsFile=$dir/known_hosts -o StrictHostKeyChecking=yes"

refs=()
for branch in development main; do
  if git rev-parse --verify --quiet "refs/remotes/origin/$branch" >/dev/null; then
    refs+=("+refs/remotes/origin/$branch:refs/heads/$branch")
  fi
done
git push "$url" "${refs[@]}" '+refs/tags/*:refs/tags/*'
