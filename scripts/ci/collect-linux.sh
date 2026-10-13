#!/usr/bin/env bash
# Gather the Linux bundles under stable names and make the portable tarball.
#
#   scripts/ci/collect-linux.sh <version> <out-dir>
#
# trade-app-<v>-linux-x86_64.tar.gz is the deb payload (usr/bin, .desktop,
# icons): the "native binary" download, and the source of the AUR package.
set -euo pipefail

version="$1"
out="$2"
bundle="${CARGO_TARGET_DIR:-.rust-target}/release/bundle"
mkdir -p "$out"

one() { # the single file matching a glob, or fail loudly
  local matches=($1)
  [[ ${#matches[@]} -eq 1 && -f "${matches[0]}" ]] || { echo "expected one file for $1, got: ${matches[*]}" >&2; exit 1; }
  echo "${matches[0]}"
}

deb="$(one "$bundle/deb/*.deb")"
cp "$deb" "$out/trade-app_${version}_amd64.deb"
cp "$(one "$bundle/rpm/*.rpm")" "$out/trade-app-${version}-1.x86_64.rpm"
cp "$(one "$bundle/appimage/*.AppImage")" "$out/TradeApp-${version}-x86_64.AppImage"
chmod +x "$out/TradeApp-${version}-x86_64.AppImage"

work="$(mktemp -d)"
deb_abs="$(realpath "$deb")"
(cd "$work" && ar x "$deb_abs" && mkdir root && tar -xf data.tar.* -C root)
root_dir="trade-app-${version}-linux-x86_64"
mv "$work/root" "$work/$root_dir"
tar -C "$work" -czf "$out/$root_dir.tar.gz" "$root_dir"
rm -rf "$work"

ls -l "$out"
