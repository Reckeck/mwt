#!/usr/bin/env bash
# Android SDK + NDK + JDK 17 and the Rust Android targets for
# `tauri android build`. Sourced after setup-linux.sh (same job shell).

export ANDROID_HOME="$CI_PROJECT_DIR/.cache/android-sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
ANDROID_PLATFORM="${ANDROID_PLATFORM:-android-35}"
ANDROID_BUILD_TOOLS="${ANDROID_BUILD_TOOLS:-35.0.0}"
ANDROID_NDK_VERSION="${ANDROID_NDK_VERSION:-27.2.12479018}" # r27c
CMDLINE_TOOLS_URL="https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"

apt-get install -y -qq --no-install-recommends openjdk-17-jdk-headless >/dev/null
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64

sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
if [[ ! -x "$sdkmanager" ]]; then
  mkdir -p "$ANDROID_HOME/cmdline-tools"
  curl -fsSL -o /tmp/cmdline-tools.zip "$CMDLINE_TOOLS_URL"
  unzip -q /tmp/cmdline-tools.zip -d /tmp/cmdline-tools
  mv /tmp/cmdline-tools/cmdline-tools "$ANDROID_HOME/cmdline-tools/latest"
fi
# `yes` dies of SIGPIPE once the licenses are accepted; that is not a failure.
(yes || true) | "$sdkmanager" --licenses >/dev/null
"$sdkmanager" --install "platform-tools" "platforms;$ANDROID_PLATFORM" \
  "build-tools;$ANDROID_BUILD_TOOLS" "ndk;$ANDROID_NDK_VERSION" >/dev/null

export NDK_HOME="$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION"
export ANDROID_NDK_HOME="$NDK_HOME"
export PATH="$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS:$ANDROID_HOME/platform-tools:$PATH"

rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
