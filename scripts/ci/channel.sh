# shellcheck shell=bash
# Sourced: the names a build ships under, by RELEASE_CHANNEL (version.sh).
# prod (main) is the release; dev (development), none and local builds are
# the dev app, which installs next to it with its own data. tauri.conf.json
# holds the dev values (lint-ci.sh checks); tauri-config.sh writes the
# channel's into the build's config overlay. app_icons is the icon folder
# under crates/app (scripts/icons.sh makes both).
if [[ "${RELEASE_CHANNEL:-dev}" == prod ]]; then
  app_name="Multi Wallet Tracker"
  app_bin=multi-wallet-tracker
  app_id=mwt.desktop
  app_android_id=mwt.app
  app_flatpak_id=mwt.app.MultiWalletTracker
  app_icons=icons/prod
else
  app_name="Multi Wallet Tracker Dev"
  app_bin=multi-wallet-tracker-dev
  app_id=dev.mwt.desktop
  app_android_id=dev.mwt.app
  app_flatpak_id=dev.mwt.MultiWalletTracker
  app_icons=icons/dev
fi
# shellcheck disable=SC2034  # read by the scripts that source this one
readonly app_name app_bin app_id app_android_id app_flatpak_id app_icons
