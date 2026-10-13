#!/usr/bin/env bash
# Build the pacman package (trade-app-bin) from the Linux tarball, in an
# archlinux:base-devel container. makepkg refuses root, so a user builds it.
#
#   scripts/ci/build-pacman.sh <version> <release-dir>
set -euo pipefail

version="$1"
release="$(realpath "$2")"
tarball="$release/trade-app-${version}-linux-x86_64.tar.gz"

pacman -Sy --noconfirm --needed base-devel >/dev/null
id builder >/dev/null 2>&1 || useradd -m builder
work="$(mktemp -d)"
cp "$tarball" "$work/"
packaging/aur/render.sh "$version" "$tarball" >"$work/PKGBUILD"
chown -R builder "$work"
# -d: runtime deps (webkit2gtk-4.1, …) are not needed to package a binary.
su builder -c "cd '$work' && makepkg -d --noconfirm"
cp "$work"/*.pkg.tar.zst "$release/"
ls -l "$release"/*.pkg.tar.zst
