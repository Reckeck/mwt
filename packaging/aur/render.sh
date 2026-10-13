#!/usr/bin/env bash
# Render PKGBUILD for multi-wallet-tracker-bin from the release tarball.
#
#   packaging/aur/render.sh <version> <tarball> [url] > PKGBUILD
#
# Without a url the source is the local file (CI builds the .pkg.tar.zst from
# it); with one it is the published download (what goes to the AUR).
set -euo pipefail

version="$1"
tarball="$2"
url="${3:-}"
sha="$(sha256sum "$tarball" | cut -d' ' -f1)"
name="$(basename "$tarball")"
source="$name"
[[ -n "$url" ]] && source="$name::$url"

sed -e "s|@PKGVER@|$version|" -e "s|@SHA256@|$sha|" -e "s|@SOURCE@|$source|" \
  "$(dirname "$0")/PKGBUILD.in"
