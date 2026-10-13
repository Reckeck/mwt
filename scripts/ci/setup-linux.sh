#!/usr/bin/env bash
# CI bootstrap on Ubuntu 22.04: system libraries for Tauri + the mise toolchain
# (node/pnpm/rust pinned in mise.toml). Sourced, so PATH sticks for the job;
# relies on the job shell running with `set -eo pipefail` (GitLab does).
#
#   source scripts/ci/setup-linux.sh            # webkit/gtk dev libs + toolchain
#   CI_SKIP_GUI_LIBS=1 source scripts/ci/setup-linux.sh   # Android: no GTK needed
#   CI_MISE_TOOLS="node pnpm" source …   # only these tools; pnpm deps only if pnpm is among them

export DEBIAN_FRONTEND=noninteractive
packages=(build-essential curl wget file git unzip xz-utils ca-certificates pkg-config jq)
if [[ -z "${CI_SKIP_GUI_LIBS:-}" ]]; then
  packages+=(libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev
    librsvg2-dev libdbus-1-dev patchelf)
fi
apt-get update -qq
apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null

# mise: everything it installs lives under the project dir so GitLab can cache it.
export MISE_DATA_DIR="$CI_PROJECT_DIR/.cache/mise"
export MISE_CACHE_DIR="$CI_PROJECT_DIR/.cache/mise-cache"
export MISE_YES=1
export MISE_TRUSTED_CONFIG_PATHS="$CI_PROJECT_DIR"
export CARGO_HOME="$CI_PROJECT_DIR/.cache/cargo"
export RUSTUP_HOME="$CI_PROJECT_DIR/.cache/rustup"
if ! command -v mise >/dev/null; then
  curl -fsSL https://mise.run | MISE_INSTALL_PATH=/usr/local/bin/mise sh >/dev/null
fi
mise install ${CI_MISE_TOOLS:-}
export PATH="$MISE_DATA_DIR/shims:$CARGO_HOME/bin:$PATH"
# mise.toml [env] (CARGO_TARGET_DIR, RUST_LOG) for every later command.
eval "$(mise env -s bash)"
# Absolute: `tauri build` runs cargo from crates/app, a relative dir would land there.
export CARGO_TARGET_DIR="$CI_PROJECT_DIR/.rust-target"

if [[ -z "${CI_MISE_TOOLS:-}" || "$CI_MISE_TOOLS" == *pnpm* ]]; then
  pnpm config set store-dir "$CI_PROJECT_DIR/.cache/pnpm" >/dev/null
  pnpm install --frozen-lockfile
fi

