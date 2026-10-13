#!/usr/bin/env bash
# Push multi-wallet-tracker-bin to the AUR for the tagged release (archlinux image).
#
#   scripts/ci/publish-aur.sh <version> <release-dir>
#
# Needs AUR_SSH_PRIVATE_KEY (CI/CD variable of type File, or the key text):
# a key added to the AUR account that maintains multi-wallet-tracker-bin. The package
# downloads the tarball from this project's Package Registry, so the project
# (or at least its packages) must be public.
set -euo pipefail

version="$1"
release="$(realpath "$2")"
name="multi-wallet-tracker-${version}-linux-x86_64.tar.gz"
url="${CI_API_V4_URL}/projects/${CI_PROJECT_ID}/packages/generic/multi-wallet-tracker/${version}/${name}"

pacman -Sy --noconfirm --needed base-devel git openssh >/dev/null

mkdir -p ~/.ssh && chmod 700 ~/.ssh
if [[ -f "${AUR_SSH_PRIVATE_KEY:?set AUR_SSH_PRIVATE_KEY}" ]]; then
  cp "$AUR_SSH_PRIVATE_KEY" ~/.ssh/aur
else
  printf '%s\n' "$AUR_SSH_PRIVATE_KEY" >~/.ssh/aur
fi
chmod 600 ~/.ssh/aur
ssh-keyscan -t ed25519,rsa aur.archlinux.org >>~/.ssh/known_hosts 2>/dev/null
export GIT_SSH_COMMAND="ssh -i $HOME/.ssh/aur -o IdentitiesOnly=yes"

id builder >/dev/null 2>&1 || useradd -m builder
work="$(mktemp -d)"
git clone ssh://aur@aur.archlinux.org/multi-wallet-tracker-bin.git "$work/aur"
packaging/aur/render.sh "$version" "$release/$name" "$url" >"$work/aur/PKGBUILD"
chown -R builder "$work"
su builder -c "cd '$work/aur' && makepkg --printsrcinfo > .SRCINFO"

cd "$work/aur"
git config user.name "${AUR_GIT_NAME:-$GITLAB_USER_NAME}"
git config user.email "${AUR_GIT_EMAIL:-$GITLAB_USER_EMAIL}"
git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
  echo "AUR already at $version"
  exit 0
fi
git commit -m "Update to $version"
git push origin HEAD:master
