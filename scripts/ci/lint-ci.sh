#!/usr/bin/env bash
# Static checks of the CI scripts, run by `mise run check-ci` and the ci:lint
# job — the mistakes that otherwise only show up on a GitLab runner:
#   - *.ps1 must be ASCII: Windows PowerShell 5.1 reads UTF-8 without a BOM as
#     cp1252, where the bytes of «—» contain 0x94 = ” and end a string early;
#   - every shell script must parse (bash -n), be executable (CI runs it
#     directly), and pass shellcheck if present.
set -uo pipefail
cd "$(dirname "$0")/../.."

failed=0
for script in scripts/ci/*.ps1; do
  if grep -nP '[^\x00-\x7F]' "$script"; then
    echo "$script: non-ASCII characters (above) break Windows PowerShell 5.1 — use ASCII" >&2
    failed=1
  fi
done
for script in scripts/ci/*.sh packaging/aur/render.sh; do
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
if command -v shellcheck >/dev/null; then
  shellcheck -S warning -x scripts/ci/*.sh packaging/aur/render.sh || failed=1
fi
if ((failed)); then
  exit 1
fi
echo "lint-ci: CI scripts ok"
