#!/usr/bin/env bash
# Static checks of the CI scripts, run by `mise run check-ci` and the ci-lint
# check — the mistakes that otherwise only show up on a CI runner:
#   - *.ps1 must be ASCII: Windows PowerShell 5.1 reads UTF-8 without a BOM as
#     cp1252, where the bytes of «—» contain 0x94 = ” and end a string early;
#     and a bare a,b among command arguments is an array, not one argument;
#   - every shell script must parse (bash -n), be executable (CI runs it
#     directly), and pass shellcheck if present;
#   - every COPY source of scripts/ci/ci.Dockerfile must be let through by
#     .dockerignore (a `*` plus an allow-list), or the image build finds nothing;
#   (the channel names against the Tauri configs: unit tests of crates/xtask)
set -uo pipefail
cd "$(dirname "$0")/../.."

failed=0
for script in scripts/ci/*.ps1; do
  [[ -e "$script" ]] || continue
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
if command -v shellcheck >/dev/null; then
  shellcheck -S warning -x scripts/ci/*.sh packaging/aur/render.sh || failed=1
fi
if ((failed)); then
  exit 1
fi
echo "lint-ci: CI scripts ok"
