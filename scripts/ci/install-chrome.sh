#!/usr/bin/env bash
# Chrome for Testing's headless shell for check-layout (Ubuntu 22.04 has no
# Chromium outside snap). Pinned: a browser update must not change the gate.
# Prints the binary's path; its runtime libraries are APT_BROWSER.
#
#   CHROMIUM="$(scripts/ci/install-chrome.sh /opt/chrome)"
set -euo pipefail
CHROME_VERSION="131.0.6778.85"
dir="${1:?usage: install-chrome.sh <dir>}"
binary="$dir/chrome-headless-shell-linux64/chrome-headless-shell"
if [[ ! -x "$binary" ]]; then
  mkdir -p "$dir"
  url="https://storage.googleapis.com/chrome-for-testing-public/$CHROME_VERSION/linux64/chrome-headless-shell-linux64.zip"
  curl -fsSL -o "$dir/chrome.zip" "$url"
  unzip -q -o "$dir/chrome.zip" -d "$dir"
  rm -f "$dir/chrome.zip"
fi
echo "$binary"
