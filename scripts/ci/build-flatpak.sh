#!/usr/bin/env bash
# The Flatpak bundle from the Linux tarball (build-linux.sh), in an
# archlinux image. flatpak-builder needs bubblewrap: the container must be
# privileged (GitLab.com shared runners are; podman run --privileged locally).
#
#   MWT_APP_NAME=... MWT_APP_FLATPAK_ID=... scripts/ci/build-flatpak.sh <version> <release-dir>
#
# The names come from the caller (`Channel::env` in crates/xtask): this container has no cargo.
set -euo pipefail
cd "$(dirname "$0")/../.."
app_name="${MWT_APP_NAME:?the channel product name}"
app_flatpak_id="${MWT_APP_FLATPAK_ID:?the channel Flatpak id}"

version="$1"
release="$2"
if ! command -v flatpak-builder >/dev/null; then
  pacman -Sy --noconfirm --needed flatpak flatpak-builder git >/dev/null
fi
rm -rf packaging/flatpak/shared-modules
git clone --depth 1 https://github.com/flathub/shared-modules.git packaging/flatpak/shared-modules
cp "$release/multi-wallet-tracker-$version-linux-x86_64.tar.gz" packaging/flatpak/multi-wallet-tracker-linux-x86_64.tar.gz
# The committed manifest and metainfo are the dev app's; another channel gets
# a copy under its own id and name (gitignored).
template=dev.mwt.MultiWalletTracker
if [[ "$app_flatpak_id" != "$template" ]]; then
  for suffix in yml metainfo.xml; do
    sed -e "s/$template/$app_flatpak_id/g" -e "s|<name>Multi Wallet Tracker Dev</name>|<name>$app_name</name>|" \
      "packaging/flatpak/$template.$suffix" >"packaging/flatpak/$app_flatpak_id.$suffix"
  done
fi
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak-builder --user --disable-rofiles-fuse --install-deps-from=flathub --force-clean \
  --repo=.flatpak-repo .flatpak-build "packaging/flatpak/$app_flatpak_id.yml"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  .flatpak-repo "$release/multi-wallet-tracker-$version-x86_64.flatpak" "$app_flatpak_id"
ls -l "$release"/*.flatpak
