#!/usr/bin/env bash
# Does the app still compile for every platform it ships on? A fast check
# (clippy, no bundles) for the code that differs per OS: #[cfg(desktop)] /
# #[cfg(mobile)], target-only dependencies, platform_fs. Run it after the last
# edit, before landing a workspace or committing (AGENTS.md, «Сборка на всех
# платформах»). Bundles are `mise run build:all`; this is minutes, not an hour.
#
#   mise run check-platforms                  # every platform
#   mise run check-platforms -- windows android
#
#   linux    host clippy (-D warnings)
#   windows  cargo xwin clippy, x86_64-pc-windows-msvc
#   android  clippy for aarch64-linux-android: the host NDK when there is one,
#            else the ci-android image through podman/docker
#   macos    on a Mac only (a Linux host cannot compile Apple targets);
#            MACOS_BUILD_HOST=user@mac runs it there over ssh
#
# A platform the machine cannot check prints SKIP with the reason: report it,
# it is not a pass. Exit 1 when any platform fails.

set -uo pipefail
cd "$(dirname "$0")/../.."
root="$PWD"

all=(linux windows android macos)
platforms=("$@")
[[ ${#platforms[@]} -eq 0 ]] && platforms=("${all[@]}")
for platform in "${platforms[@]}"; do
  case "$platform" in
    linux | windows | android | macos) ;;
    *) echo "unknown platform: $platform (one of: ${all[*]})" >&2; exit 2 ;;
  esac
done

log_dir="$root/.cache/check-platforms"
mkdir -p "$log_dir"
lints=(-- -D warnings)

# Builds regenerate tauri's committed schemas (crates/app/gen/schemas): put
# them back as they were, so a check leaves no diff behind.
schemas="$(mktemp -d)"
cp -a crates/app/gen/schemas/. "$schemas/"
trap 'rm -rf crates/app/gen/schemas && mkdir -p crates/app/gen && mv "$schemas" crates/app/gen/schemas && chmod 755 crates/app/gen/schemas' EXIT

skip() { echo "skipped: $*"; return 3; }

check_linux() {
  cargo clippy --workspace --all-targets "${lints[@]}"
}

check_windows() {
  command -v cargo-xwin >/dev/null || skip "no cargo-xwin (cargo install --locked cargo-xwin)" || return
  rustup target list --installed | grep -qx x86_64-pc-windows-msvc ||
    skip "no Rust target (rustup target add x86_64-pc-windows-msvc)" || return
  cargo xwin clippy -p trade-app --lib --target x86_64-pc-windows-msvc "${lints[@]}"
}

# The NDK's clang for C dependencies built by build scripts.
ndk_env() {
  local ndk="$1" bin
  bin="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
  [[ -x "$bin/aarch64-linux-android24-clang" ]] || return 1
  export CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang"
  export AR_aarch64_linux_android="$bin/llvm-ar"
  export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
}

check_android() {
  local ndk="${ANDROID_NDK_HOME:-${NDK_HOME:-}}"
  if [[ -n "$ndk" ]] && rustup target list --installed | grep -qx aarch64-linux-android && ndk_env "$ndk"; then
    cargo clippy -p trade-app --lib --target aarch64-linux-android "${lints[@]}"
    return
  fi
  local engine image_tag image checkout
  engine="${CONTAINER_ENGINE:-$(command -v podman || command -v docker || true)}"
  [[ -n "$engine" ]] || skip "no Android NDK on the host (ANDROID_NDK_HOME) and no podman/docker for the ci-android image" || return
  image_tag="$(sed -n 's/^  CI_IMAGE_TAG: "\(.*\)".*/\1/p' .gitlab-ci.yml)"
  image="${CI_REGISTRY_IMAGE:-registry.gitlab.com/reckeck/trade-app}/ci-android:$image_tag"
  if ! "$engine" image inspect "$image" >/dev/null 2>&1; then
    "$engine" pull -q "$image" || skip "no ci-android image $image (mise run build:all -- android builds it)" || return
  fi
  # Same volumes as build-all.sh, so the compiled crates are reused.
  checkout="$(printf '%s' "$root" | sha256sum | cut -c1-8)"
  "$engine" run --rm --init \
    -v "$root:/src" -w /src \
    -v "mwt-build-$checkout-android-target:/src/.rust-target" \
    -v "mwt-build-$checkout-android-home:/root" \
    -e CI_PROJECT_DIR=/src -e CI=true -e CI_SKIP_GUI_LIBS=1 \
    "$image" bash -c 'set -eo pipefail
      source scripts/ci/setup-linux.sh; source scripts/ci/setup-android.sh
      ndk="${ANDROID_NDK_HOME:-$NDK_HOME}"; bin="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
      export CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang" AR_aarch64_linux_android="$bin/llvm-ar"
      export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
      cargo clippy -p trade-app --lib --target aarch64-linux-android -- -D warnings'
}

check_macos() {
  if [[ "$(uname -s)" == Darwin ]]; then
    cargo clippy -p trade-app --lib "${lints[@]}"
    return
  fi
  [[ -n "${MACOS_BUILD_HOST:-}" ]] ||
    skip "macOS compiles on a Mac only; set MACOS_BUILD_HOST=user@mac to check there over ssh" || return
  local remote="${MACOS_BUILD_DIR:-mwt-build}"
  rsync -a --delete --exclude-from=<(printf '%s\n' .git .jj .rust-target node_modules .cache release dist \
    crates/app/gen .flatpak-build .flatpak-repo .flatpak-builder) ./ "$MACOS_BUILD_HOST:$remote/" &&
    ssh "$MACOS_BUILD_HOST" "cd '$remote' && bash -lc 'cargo clippy -p trade-app --lib -- -D warnings'"
}

declare -A result
for platform in "${platforms[@]}"; do
  log="$log_dir/$platform.log"
  echo "==> $platform, log: ${log#"$root"/}"
  start=$(date +%s)
  "check_$platform" >"$log" 2>&1
  code=$?
  took="$(($(date +%s) - start))s"
  case "$code" in
    0) result[$platform]="OK      $took" ;;
    3) result[$platform]="SKIP    $(tail -n1 "$log" | sed 's/^skipped: //')" ;;
    *) result[$platform]="FAIL    $took, see ${log#"$root"/}"; grep -E '^(error|warning)' -A6 "$log" | head -n 30 | sed 's/^/    /' ;;
  esac
  echo "    ${result[$platform]}"
done

echo
failed=0
for platform in "${platforms[@]}"; do
  printf '  %-8s %s\n' "$platform" "${result[$platform]}"
  [[ "${result[$platform]}" == FAIL* ]] && failed=1
done
exit "$failed"
