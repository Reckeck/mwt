#!/usr/bin/env bash
# Static checks of the CI scripts, run by `mise run check-ci` and the ci:lint
# job — the mistakes that otherwise only show up on a GitLab runner:
#   - *.ps1 must be ASCII: Windows PowerShell 5.1 reads UTF-8 without a BOM as
#     cp1252, where the bytes of «—» contain 0x94 = ” and end a string early;
#     and a bare a,b among command arguments is an array, not one argument;
#   - every shell script must parse (bash -n), be executable (CI runs it
#     directly), and pass shellcheck if present;
#   - every COPY source of scripts/ci/ci.Dockerfile must be let through by
#     .dockerignore (a `*` plus an allow-list), or the image build finds nothing;
#   - the channel names of channel.sh: the dev ones are what the Tauri configs
#     hold, and build-windows.ps1 repeats both channels' exactly.
set -uo pipefail
cd "$(dirname "$0")/../.."

failed=0
for script in scripts/ci/*.ps1; do
  if grep -nP '[^\x00-\x7F]' "$script"; then
    echo "$script: non-ASCII characters (above) break Windows PowerShell 5.1 — use ASCII" >&2
    failed=1
  fi
  # A bare a,b among command arguments is an array in PowerShell: the program
  # gets "a b". Quote such lists ('nsis,msi').
  if grep -nP "^\s*(Invoke-\w+|&)\s.*\s(?<!['\"])[\w.-]+,[\w.-]+(?=\s|$)" "$script"; then
    echo "$script: unquoted comma list in a command (above) — quote it" >&2
    failed=1
  fi
done
for script in scripts/ci/*.sh scripts/local/*.sh packaging/aur/render.sh packaging/linux/*.sh; do
  if ! bash -n "$script"; then
    echo "$script: does not parse" >&2
    failed=1
  fi
  # CI runs them directly: without the bit a job dies with "Permission denied".
  if [[ ! -x "$script" ]]; then
    echo "$script: not executable, chmod +x it" >&2
    failed=1
  fi
done
for source in $(awk '/^COPY / && !/--from=/ {print $2}' scripts/ci/ci.Dockerfile); do
  if ! grep -qxF "!$source" .dockerignore; then
    echo "scripts/ci/ci.Dockerfile copies $source, but .dockerignore filters it out — add !$source" >&2
    failed=1
  fi
done
# shellcheck source=scripts/ci/channel.sh
for channel in dev prod; do
  (
    RELEASE_CHANNEL="$channel" source scripts/ci/channel.sh
    for value in "'$app_id'" "'$app_name'" "'$app_bin'"; do
      grep -qF "$value" scripts/ci/build-windows.ps1 ||
        { echo "scripts/ci/build-windows.ps1 lacks $value of channel $channel (channel.sh)" >&2; exit 1; }
    done
    [[ "$channel" == dev ]] || exit 0
    for pair in "identifier:$app_id" "productName:$app_name" "mainBinaryName:$app_bin"; do
      grep -qF "\"${pair%%:*}\": \"${pair#*:}\"" crates/app/tauri.conf.json ||
        { echo "crates/app/tauri.conf.json: ${pair%%:*} is not the dev channel's ${pair#*:} (channel.sh)" >&2; exit 1; }
    done
    grep -qF "\"identifier\": \"$app_android_id\"" crates/app/tauri.android.conf.json ||
      { echo "crates/app/tauri.android.conf.json: identifier is not $app_android_id (channel.sh)" >&2; exit 1; }
  ) || failed=1
done
if command -v shellcheck >/dev/null; then
  shellcheck -S warning -x scripts/ci/*.sh packaging/aur/render.sh || failed=1
fi
if ((failed)); then
  exit 1
fi
echo "lint-ci: CI scripts ok"
