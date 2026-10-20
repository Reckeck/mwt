#!/usr/bin/env bash
# Ubuntu 22.04 packages of the CI jobs, in one place: the CI image
# (scripts/ci/ci.Dockerfile) bakes them in, setup-linux.sh installs them when
# a job runs without that image. Sourced.

# shellcheck disable=SC2034  # read by the scripts that source this file
# libatomic1: Node 26 needs it, and nothing else in the core set pulls it in.
APT_CORE=(curl wget file git unzip xz-utils ca-certificates jq libatomic1)
APT_BUILD=(build-essential pkg-config)
# Tauri on Linux: WebKitGTK, tray, keyring (D-Bus), AppImage tooling.
APT_GUI=(libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev
  librsvg2-dev libdbus-1-dev patchelf)
APT_ANDROID=(openjdk-17-jdk-headless)
# check-layout: what Chrome for Testing's headless shell links against (scripts/ci/install-chrome.sh),
# and a font with Cyrillic for the screenshots.
APT_BROWSER=(libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libcups2 libdrm2 libxkbcommon0 libxcomposite1
  libxdamage1 libxfixes3 libxrandr2 libgbm1 libasound2 libpango-1.0-0 libcairo2 libdbus-1-3 fonts-dejavu-core)
