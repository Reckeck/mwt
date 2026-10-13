#!/usr/bin/env bash
# The version a build ships under, and the checks that keep it honest.
#
#   scripts/ci/version.sh                         # the app version X.Y.Z
#   scripts/ci/version.sh --channel dev|prod|none [--dotenv FILE]
#
# Channels (see docs/workflow.md):
#   prod  X.Y.Z              main: a release; tag vX.Y.Z must not exist yet
#   dev   X.Y.Z-<pipeline>   development: a pre-release of the coming X.Y.Z;
#                            once X.Y.Z is released (and `mise run bump` has
#                            not run yet) of X.Y.(Z+1)
#   none  X.Y.Z-<pipeline>   builds that publish nothing (manual, scheduled)
# The pre-release part is numeric: MSI accepts nothing else (max 65535).
# --dotenv writes BUILD_VERSION, ANDROID_VERSION_CODE and RELEASE_CHANNEL for
# the jobs that need this one (GitLab dotenv report).
set -euo pipefail
cd "$(dirname "$0")/../.."

channel=""
dotenv=""
while (($#)); do
  case "$1" in
    --channel) channel="$2"; shift 2 ;;
    --dotenv) dotenv="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

version="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' crates/app/tauri.conf.json)"
cargo_version="$(sed -n '/^\[workspace.package\]/,/^\[/ s/^version = "\(.*\)"$/\1/p' Cargo.toml)"
[[ -n "$version" ]] || { echo "no version in crates/app/tauri.conf.json" >&2; exit 1; }
if [[ -z "$channel" ]]; then
  echo "$version"
  exit 0
fi

if [[ "$cargo_version" != "$version" ]]; then
  echo "Cargo.toml workspace version $cargo_version differs from tauri.conf.json $version" >&2
  exit 1
fi
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version $version must be plain X.Y.Z; CI adds the pre-release part itself" >&2
  exit 1
fi

pipeline="${CI_PIPELINE_IID:-1}"
case "$channel" in
  prod) build="$version" ;;
  dev | none)
    if ((pipeline > 65535)); then
      echo "pipeline number $pipeline exceeds what an MSI version holds (65535)" >&2
      exit 1
    fi
    build="$version-$pipeline"
    ;;
  *) echo "channel must be dev, prod or none, not '$channel'" >&2; exit 2 ;;
esac

# A released version is final: main must move on (mise run bump), and the
# pre-releases of development aim past it until the next bump.
if [[ "$channel" != none ]]; then
  remote="${CI_REPOSITORY_URL:-origin}"
  if git ls-remote --exit-code --tags "$remote" "refs/tags/v$version" >/dev/null 2>&1; then
    if [[ "$channel" == prod ]]; then
      echo "v$version is already released: run \`mise run bump\` on development before merging into main" >&2
      exit 1
    fi
    IFS=. read -r major minor patch <<<"$version"
    build="$major.$minor.$((patch + 1))-$pipeline"
    echo "v$version is released; this pre-release is of $major.$minor.$((patch + 1))" >&2
  fi
fi
# A release carries its CHANGELOG section (mise run bump writes it).
if [[ "$channel" == prod ]] && ! grep -q "^## \[$version\]" CHANGELOG.md; then
  echo "CHANGELOG.md has no «## [$version]» section: run \`mise run bump\` on development" >&2
  exit 1
fi

if [[ -n "$dotenv" ]]; then
  {
    echo "BUILD_VERSION=$build"
    echo "ANDROID_VERSION_CODE=$pipeline"
    echo "RELEASE_CHANNEL=$channel"
  } >"$dotenv"
fi
echo "$build"
