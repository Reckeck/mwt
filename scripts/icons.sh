#!/usr/bin/env bash
# App icons of both channels from the SVG sources in crates/app/icons/src:
# crates/app/icons/prod (release) and crates/app/icons/dev (dev and local
# builds, the default in tauri.conf.json). The PNGs are committed; CI renders
# nothing, it only picks the channel's folder (crates/xtask channel.rs, tauri_config.rs).
#
#   mise run icons
#
# Needs rsvg-convert (librsvg) and ImageMagick 7 (magick). 32 px and below
# come from the *-small sources: the full mark turns to mush there.
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in rsvg-convert magick; do
  command -v "$tool" >/dev/null || { echo "icons: $tool not found" >&2; exit 1; }
done

src=crates/app/icons/src
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

render() { rsvg-convert -w "$2" -h "$2" "$1" -o "$3"; }

for channel in prod dev; do
  suffix=""
  [[ "$channel" == dev ]] && suffix=-dev
  out="crates/app/icons/$channel"
  render "$src/icon$suffix.svg" 1024 "$tmp/icon.png"
  rm -rf "$out"
  pnpm --silent tauri icon "$tmp/icon.png" -o "$out" >"$tmp/log" 2>&1 || { cat "$tmp/log" >&2; exit 1; }
  # Not shipped: the generic Android set (build-android.sh makes the adaptive
  # one), iOS (no target) and Appx logos (no MSIX bundle).
  rm -rf "$out/android" "$out/ios" "$out"/Square*Logo.png "$out/StoreLogo.png"

  for size in 16 24 32; do render "$src/icon$suffix-small.svg" "$size" "$tmp/small-$size.png"; done
  for size in 48 64 256; do render "$src/icon$suffix.svg" "$size" "$tmp/big-$size.png"; done
  cp "$tmp/small-32.png" "$out/32x32.png"
  magick "$tmp"/small-{16,24,32}.png "$tmp"/big-{48,64,256}.png "$out/icon.ico"

  # Android adaptive icon: build-android.sh runs `tauri icon` on this manifest.
  mkdir -p "$out/android"
  render "$src/android-background.svg" 1024 "$out/android/background.png"
  render "$src/android-foreground$suffix.svg" 1024 "$out/android/foreground.png"
  render "$src/android-monochrome.svg" 1024 "$out/android/monochrome.png"
  printf '{"default":"../icon.png","android_bg":"background.png","android_fg":"foreground.png","android_fg_scale":100,"android_monochrome":"monochrome.png"}\n' \
    >"$out/android/manifest.json"
done

mkdir -p public
cp "$src/icon-small.svg" public/favicon.svg
echo "icons: crates/app/icons/{prod,dev}, public/favicon.svg"
