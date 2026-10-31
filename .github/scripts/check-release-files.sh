#!/usr/bin/env bash
# The files of a release (decision 12 of the build rewrite plan) are all in the directory, and
# none of them is signed with the throwaway test key.
#
#   .github/scripts/check-release-files.sh <dir> <version> [macos]
#
#   Linux    .deb .rpm .AppImage, linux-x86_64.tar.gz (the AUR source), .pkg.tar.zst (pacman), .flatpak
#   Android  universal APK and AAB
#   Windows  NSIS setup.exe, an .msi per language, the portable zip
#   macOS    universal .dmg and the .app tarball (only with the third argument)
set -euo pipefail

dir="${1:?release directory}"
version="${2:?version}"
prefix="multi-wallet-tracker"

missing=()
want() { [[ -f "$dir/$1" ]] || missing+=("$1"); }
want_glob() {
  local matches=()
  mapfile -t matches < <(compgen -G "$dir/$1" || true)
  [[ ${#matches[@]} -ge 1 ]] || missing+=("$1")
}

want "${prefix}_${version}_amd64.deb"
want "${prefix}-${version}-1.x86_64.rpm"
want "${prefix}-${version}-x86_64.AppImage"
want "${prefix}-${version}-linux-x86_64.tar.gz"
want "${prefix}-${version}-x86_64.flatpak"
want_glob "*.pkg.tar.zst"
want "${prefix}-${version}-android-universal.apk"
want "${prefix}-${version}-android-universal.aab"
want "${prefix}-${version}-x64-setup.exe"
want_glob "${prefix}-${version}-x64-*.msi"
want "${prefix}-${version}-windows-x64-portable.zip"
if [[ "${3:-}" == macos ]]; then
  want "${prefix}-${version}-macos-universal.dmg"
  want "${prefix}-${version}-macos-universal.app.tar.gz"
fi

if ((${#missing[@]})); then
  printf 'missing from %s: %s\n' "$dir" "${missing[*]}" >&2
  exit 1
fi
if compgen -G "$dir/*testkey*" >/dev/null; then
  echo "a build signed with the throwaway Android test key must not be published:" >&2
  ls "$dir"/*testkey* >&2
  exit 1
fi
echo "release files ok:"
ls -l "$dir"
