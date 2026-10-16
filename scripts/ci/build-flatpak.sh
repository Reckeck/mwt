#!/usr/bin/env bash
# The Flatpak bundle from the Linux tarball (build-linux.sh), in an
# archlinux image. flatpak-builder needs bubblewrap: the container must be
# privileged (GitLab.com shared runners are; podman run --privileged locally).
#
#   scripts/ci/build-flatpak.sh <version> <release-dir>
set -euo pipefail
cd "$(dirname "$0")/../.."

version="$1"
release="$2"
if ! command -v flatpak-builder >/dev/null; then
  pacman -Sy --noconfirm --needed flatpak flatpak-builder git >/dev/null
fi
rm -rf packaging/flatpak/shared-modules
git clone --depth 1 https://github.com/flathub/shared-modules.git packaging/flatpak/shared-modules
cp "$release/multi-wallet-tracker-$version-linux-x86_64.tar.gz" packaging/flatpak/multi-wallet-tracker-linux-x86_64.tar.gz
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak-builder --user --disable-rofiles-fuse --install-deps-from=flathub --force-clean \
  --repo=.flatpak-repo .flatpak-build packaging/flatpak/dev.mwt.MultiWalletTracker.yml
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  .flatpak-repo "$release/multi-wallet-tracker-$version-x86_64.flatpak" dev.mwt.MultiWalletTracker
ls -l "$release"/*.flatpak
