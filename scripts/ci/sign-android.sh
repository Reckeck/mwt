#!/usr/bin/env bash
# Sign the release APK (zipalign + apksigner) and AAB (jarsigner).
#
#   scripts/ci/sign-android.sh <version> <out-dir>
#
# Release key from masked CI/CD variables:
#   ANDROID_KEYSTORE_BASE64   base64 of the .jks/.keystore file
#   ANDROID_KEYSTORE_PASSWORD store password
#   ANDROID_KEY_ALIAS         key alias
#   ANDROID_KEY_PASSWORD      key password (defaults to the store password)
# Without them a throwaway key signs the APK so it installs for testing; such
# builds cannot update each other and must not be published.
set -euo pipefail

version="$1"
out="$2"
outputs="crates/app/gen/android/app/build/outputs"
apk_in="$(find "$outputs/apk" -name '*universal-release*.apk' | head -n1)"
aab_in="$(find "$outputs/bundle" -name '*universal-release*.aab' | head -n1)"
[[ -n "$apk_in" && -n "$aab_in" ]] || { echo "release APK/AAB not found under $outputs" >&2; exit 1; }

keystore="$(mktemp -d)/release.keystore"
if [[ -n "${ANDROID_KEYSTORE_BASE64:-}" ]]; then
  echo "$ANDROID_KEYSTORE_BASE64" | base64 -d >"$keystore"
  suffix=""
else
  echo "WARNING: no ANDROID_KEYSTORE_BASE64 — signing with a throwaway test key" >&2
  export ANDROID_KEYSTORE_PASSWORD="test-only-password"
  export ANDROID_KEY_ALIAS="test"
  keytool -genkeypair -keystore "$keystore" -storepass "$ANDROID_KEYSTORE_PASSWORD" \
    -alias "$ANDROID_KEY_ALIAS" -keyalg RSA -keysize 2048 -validity 365 \
    -dname "CN=Multi Wallet Tracker test build" >/dev/null 2>&1
  suffix="-testkey"
fi
export ANDROID_KEY_PASSWORD="${ANDROID_KEY_PASSWORD:-$ANDROID_KEYSTORE_PASSWORD}"

mkdir -p "$out"
apk_out="$out/multi-wallet-tracker-${version}-android-universal${suffix}.apk"
aab_out="$out/multi-wallet-tracker-${version}-android-universal${suffix}.aab"

zipalign -p -f 4 "$apk_in" "$apk_out.aligned"
apksigner sign --ks "$keystore" --ks-key-alias "$ANDROID_KEY_ALIAS" \
  --ks-pass env:ANDROID_KEYSTORE_PASSWORD --key-pass env:ANDROID_KEY_PASSWORD \
  --out "$apk_out" "$apk_out.aligned"
rm -f "$apk_out.aligned" "$apk_out.idsig"
apksigner verify "$apk_out"

cp "$aab_in" "$aab_out"
jarsigner -sigalg SHA256withRSA -digestalg SHA-256 -keystore "$keystore" \
  -storepass:env ANDROID_KEYSTORE_PASSWORD -keypass:env ANDROID_KEY_PASSWORD \
  "$aab_out" "$ANDROID_KEY_ALIAS" >/dev/null

rm -f "$keystore"
ls -l "$out"
