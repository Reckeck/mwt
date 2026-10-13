#!/usr/bin/env bash
# CI bootstrap for the Linux jobs. Sourced, so PATH sticks for the job;
# relies on the job shell running with `set -eo pipefail` (GitLab does).
#
# In the CI image (scripts/ci/ci.Dockerfile) everything is baked in and this
# only wires caches; on a bare ubuntu:22.04 (CI_*_IMAGE overridden) it
# installs the same packages and toolchain itself, just slower.
#
#   source scripts/ci/setup-linux.sh            # webkit/gtk dev libs + toolchain
#   CI_SKIP_GUI_LIBS=1 source scripts/ci/setup-linux.sh   # no GTK (Android)
#   CI_MISE_TOOLS="node pnpm" source …   # only these tools; pnpm deps only if pnpm is among them

export MISE_YES=1
export MISE_TRUSTED_CONFIG_PATHS="$CI_PROJECT_DIR"

if [[ -f /etc/mwt-ci-image ]]; then
  # The tools are baked in; `mise run` would otherwise re-run rustup first.
  export MISE_TASK_RUN_AUTO_INSTALL=false
  # Cargo's download cache lives in the project so GitLab can keep it.
  mkdir -p "$CI_PROJECT_DIR/.cache/cargo-registry"
  rm -rf "$CARGO_HOME/registry"
  ln -s "$CI_PROJECT_DIR/.cache/cargo-registry" "$CARGO_HOME/registry"
else
  export DEBIAN_FRONTEND=noninteractive
  # shellcheck source=scripts/ci/apt-packages.sh
  source "$CI_PROJECT_DIR/scripts/ci/apt-packages.sh"
  packages=("${APT_CORE[@]}" "${APT_BUILD[@]}")
  if [[ -z "${CI_SKIP_GUI_LIBS:-}" ]]; then
    packages+=("${APT_GUI[@]}")
  fi
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null
  export MISE_DATA_DIR="$CI_PROJECT_DIR/.cache/mise"
  export MISE_CACHE_DIR="$CI_PROJECT_DIR/.cache/mise-cache"
  export CARGO_HOME="$CI_PROJECT_DIR/.cache/cargo"
  export RUSTUP_HOME="$CI_PROJECT_DIR/.cache/rustup"
  if ! command -v mise >/dev/null; then
    curl -fsSL https://mise.run | MISE_INSTALL_PATH=/usr/local/bin/mise sh >/dev/null
  fi
fi

# Only when something is missing: `mise install` re-runs rustup (and brings
# rust-docs back) even for a toolchain the image already has.
if [[ -n "$(mise ls --missing 2>/dev/null)" ]]; then
  # shellcheck disable=SC2086  # a space-separated tool list
  mise install ${CI_MISE_TOOLS:-}
fi
export PATH="$MISE_DATA_DIR/shims:$CARGO_HOME/bin:$PATH"
# mise.toml [env] (CARGO_TARGET_DIR, RUST_LOG) for every later command.
eval "$(mise env -s bash)"
# Absolute: `tauri build` runs cargo from crates/app, a relative dir would land there.
export CARGO_TARGET_DIR="$CI_PROJECT_DIR/.rust-target"

# Compiled crates come back from the GitLab cache through sccache.
if command -v sccache >/dev/null; then
  export RUSTC_WRAPPER=sccache
  export SCCACHE_DIR="$CI_PROJECT_DIR/.cache/sccache"
  export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-3G}"
  export CARGO_INCREMENTAL=0
  sccache --start-server >/dev/null 2>&1 || true
fi

if [[ -z "${CI_MISE_TOOLS:-}" || "$CI_MISE_TOOLS" == *pnpm* ]]; then
  pnpm config set store-dir "$CI_PROJECT_DIR/.cache/pnpm" >/dev/null
  pnpm install --frozen-lockfile --prefer-offline
fi
