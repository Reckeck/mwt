#!/usr/bin/env bash
# Build the app for every platform on this machine, with the scripts and
# images CI uses (scripts/ci/*), so a local bundle is what CI would ship.
#
#   scripts/local/build-all.sh [--version X.Y.Z] [--native] [platform...]
#   mise run build:all -- linux android
#
# Platforms (default: all of them):
#   linux    deb, rpm, AppImage, tarball  ci-linux image
#   pacman   .pkg.tar.zst (after linux)   archlinux image
#   flatpak  .flatpak (after linux)       archlinux image, --privileged
#   android  APK + AAB, test key          ci-android image
#   windows  NSIS installer + portable    ci-linux + cargo-xwin (no MSI: WiX
#                                         runs on Windows only, CI makes it)
#   macos    universal .dmg               this Mac, or MACOS_BUILD_HOST=user@mac
#                                         over ssh; otherwise skipped
#
# Containers run with podman (rootless: files stay yours) or docker
# (CONTAINER_ENGINE=docker). --native builds linux/flatpak on this host with
# its own toolchain instead. Bundles land in release/; the build directories
# of each container are volumes (mwt-build-<checkout>-*), so the host .rust-target and
# node_modules stay untouched. Android signs with the release key when the
# ANDROID_KEYSTORE_* variables are set (sign-android.sh), else a test key.
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1
root="$PWD"

all=(linux pacman flatpak android windows macos)
version=""
native=""
platforms=()
while (($#)); do
  case "$1" in
    --version) version="$2"; shift 2 ;;
    --native) native=1; shift ;;
    -h | --help) sed -n '2,/^set -uo/{/^set -uo/d;s/^# \{0,1\}//;p}' "$0"; exit 0 ;;
    linux | pacman | flatpak | android | windows | macos) platforms+=("$1"); shift ;;
    *) echo "unknown argument: $1 (platforms: ${all[*]})" >&2; exit 2 ;;
  esac
done
((${#platforms[@]})) || platforms=("${all[@]}")
version="${version:-$(scripts/ci/version.sh)}"
out=release

engine="${CONTAINER_ENGINE:-$(command -v podman >/dev/null && echo podman || echo docker)}"
image_tag="$(sed -n 's/^  CI_IMAGE_TAG: "\(.*\)".*/\1/p' .gitlab-ci.yml)"
registry="${CI_REGISTRY_IMAGE:-registry.gitlab.com/reckeck/trade-app}"
arch_image=docker.io/library/archlinux:base-devel

declare -A result
log_dir="$root/.cache/build-all"
# Volumes per checkout: cargo trusts file times, and another checkout's
# sources (older files) would look unchanged next to its artifacts.
checkout="$(printf '%s' "$root" | sha256sum | cut -c1-8)"
mkdir -p "$log_dir" "$out"

ci_image() { # kind (linux|android): the CI image, pulled, or built if the registry lacks it
  local image="$registry/ci-$1:$image_tag"
  if ! "$engine" image exists "$image" 2>/dev/null && ! "$engine" image inspect "$image" >/dev/null 2>&1; then
    "$engine" pull -q "$image" >&2 ||
      "$engine" build --target "$1" -f scripts/ci/ci.Dockerfile -t "$image" . >&2 || return 1
  fi
  echo "$image"
}

# run_ci <image> <volume-name> <command>: the command in a CI image with the
# project at /src, as a GitLab job would see it.
run_ci() {
  local image="$1" volume="$2" command="$3"
  "$engine" run --rm --init \
    -v "$root:/src" -w /src \
    -v "mwt-build-$checkout-$volume-target:/src/.rust-target" \
    -v "mwt-build-$checkout-$volume-node:/src/node_modules" \
    -v "mwt-build-$checkout-$volume-home:/root" \
    -e CI_PROJECT_DIR=/src -e CI=true -e GRADLE_USER_HOME=/src/.cache/gradle \
    -e ANDROID_TARGETS -e ANDROID_KEYSTORE_BASE64 -e ANDROID_KEYSTORE_PASSWORD \
    -e ANDROID_KEY_ALIAS -e ANDROID_KEY_PASSWORD \
    "$image" bash -c "set -eo pipefail; source scripts/ci/setup-linux.sh; mkdir -p dist; $command"
}

run_arch() { # <command> [engine args...]: the command in archlinux:base-devel
  local command="$1"
  shift
  "$engine" run --rm --init "$@" -v "$root:/src" -w /src "$arch_image" bash -c "set -eo pipefail; $command"
}

tarball="$out/multi-wallet-tracker-$version-linux-x86_64.tar.gz"
need_tarball() {
  [[ -f "$tarball" ]] || { echo "no $tarball: build linux first (build-all.sh linux $1)" >&2; return 1; }
}

build_linux() {
  if [[ -n "$native" ]]; then
    scripts/ci/build-linux.sh "$version" "$out"
  else
    local image
    image="$(ci_image linux)" || return 1
    run_ci "$image" linux "scripts/ci/build-linux.sh '$version' '$out'"
  fi
}

build_pacman() {
  need_tarball pacman || return 1
  run_arch "scripts/ci/build-pacman.sh '$version' '$out'"
}

build_flatpak() {
  need_tarball flatpak || return 1
  if [[ -n "$native" ]] && command -v flatpak-builder >/dev/null; then
    scripts/ci/build-flatpak.sh "$version" "$out"
  else
    run_arch "scripts/ci/build-flatpak.sh '$version' '$out'" --privileged
  fi
}

build_android() {
  local image
  image="$(ci_image android)" || return 1
  run_ci "$image" android "source scripts/ci/setup-android.sh; scripts/ci/build-android.sh '$version' '$out'"
}

build_windows() {
  local base image
  base="$(ci_image linux)" || return 1
  image="localhost/mwt-windows-cross:$image_tag"
  if ! "$engine" image inspect "$image" >/dev/null 2>&1; then
    "$engine" build --build-arg "BASE=$base" -f scripts/local/windows-cross.Dockerfile -t "$image" scripts/local || return 1
  fi
  run_ci "$image" windows "scripts/local/build-windows-cross.sh '$version' '$out'"
}

build_macos() {
  if [[ "$(uname -s)" == Darwin ]]; then
    scripts/ci/build-macos.sh "$version" "$out"
    return
  fi
  if [[ -z "${MACOS_BUILD_HOST:-}" ]]; then
    echo "skipped: macOS builds on a Mac only; set MACOS_BUILD_HOST=user@mac to build there over ssh"
    return 3
  fi
  local remote="${MACOS_BUILD_DIR:-mwt-build}"
  rsync -a --delete --exclude-from=<(printf '%s\n' .git .jj .rust-target node_modules .cache release dist \
    crates/app/gen .flatpak-build .flatpak-repo .flatpak-builder) ./ "$MACOS_BUILD_HOST:$remote/" &&
    ssh "$MACOS_BUILD_HOST" "cd '$remote' && bash -lc \"scripts/ci/build-macos.sh '$version' release\"" &&
    rsync -a "$MACOS_BUILD_HOST:$remote/release/" "$out/" --include='*macos*' --exclude='*'
}

# Builds regenerate tauri's committed schemas (crates/app/gen/schemas, with
# the Android and Windows ones): put them back as they were.
schemas="$(mktemp -d)"
cp -a crates/app/gen/schemas/. "$schemas/"
trap 'rm -rf crates/app/gen/schemas && mkdir -p crates/app/gen && mv "$schemas" crates/app/gen/schemas && chmod 755 crates/app/gen/schemas' EXIT

for platform in "${platforms[@]}"; do
  log="$log_dir/$platform.log"
  echo "==> $platform ($version), log: ${log#"$root"/}"
  start=$(date +%s)
  "build_$platform" >"$log" 2>&1
  code=$?
  took="$(($(date +%s) - start))s"
  case "$code" in
    0) result[$platform]="OK      $took" ;;
    3) result[$platform]="SKIP    $(tail -n1 "$log")" ;;
    *) result[$platform]="FAIL    $took, see ${log#"$root"/}"; tail -n 15 "$log" | sed 's/^/    /' ;;
  esac
  echo "    ${result[$platform]}"
done

echo
echo "Multi Wallet Tracker $version -> $out/"
failed=0
for platform in "${platforms[@]}"; do
  printf '  %-8s %s\n' "$platform" "${result[$platform]}"
  [[ "${result[$platform]}" == FAIL* ]] && failed=1
done
ls -1 "$out" | sed 's/^/    /'
exit "$failed"
