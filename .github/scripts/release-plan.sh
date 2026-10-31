#!/usr/bin/env bash
# What a release.yml run builds and whether it publishes. Writes to $GITHUB_OUTPUT (stdout off CI):
#   channel   dev | prod         (the names the build ships under)
#   publish   true | false       (a GitHub release is created)
#   pre       true | false       (a pre-release X.Y.Z-N instead of a release X.Y.Z)
#   kind      dev | prod | none  (`cargo xtask version --channel`: none = a build that publishes nothing)
#
# Push: development -> dev pre-release, main -> prod release. By hand (workflow_dispatch):
# the chosen channel; it publishes only from its own branch (dev from development, prod from main).
#
#   EVENT_NAME=push REF_NAME=development .github/scripts/release-plan.sh
set -euo pipefail

event="${EVENT_NAME:?EVENT_NAME}"
ref="${REF_NAME:?REF_NAME}"

case "$event" in
  push)
    case "$ref" in
      development) channel=dev ;;
      main) channel=prod ;;
      *) echo "release.yml publishes from development and main only, not $ref" >&2; exit 1 ;;
    esac
    publish=true
    ;;
  workflow_dispatch)
    channel="${INPUT_CHANNEL:-dev}"
    publish="${INPUT_PUBLISH:-false}"
    case "$channel" in dev | prod) ;; *) echo "channel must be dev or prod, not $channel" >&2; exit 1 ;; esac
    case "$publish" in true | false) ;; *) echo "publish must be true or false, not $publish" >&2; exit 1 ;; esac
    if [[ "$publish" == true ]]; then
      if [[ "$channel" == dev && "$ref" != development ]] || [[ "$channel" == prod && "$ref" != main ]]; then
        echo "publish needs the dev channel on development or the prod channel on main (this run: $channel on $ref)" >&2
        exit 1
      fi
    fi
    ;;
  *)
    echo "release.yml does not run on $event" >&2
    exit 1
    ;;
esac

if [[ "$publish" == true || "$channel" == prod ]]; then
  kind="$channel"
else
  kind=none
fi
pre=false
if [[ "$publish" == true && "$channel" == dev ]]; then
  pre=true
fi

{
  echo "channel=$channel"
  echo "publish=$publish"
  echo "pre=$pre"
  echo "kind=$kind"
} >>"${GITHUB_OUTPUT:-/dev/stdout}"
