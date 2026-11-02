# CI images, one per kind of job, so a job starts with its toolchain instead
# of installing it (GitHub-hosted runners are fresh VMs every time):
#   node     mise + node/pnpm from mise.toml,        frontend checks
#            headless Chrome for check-layout
#   linux    + build tools, Tauri libraries, Rust,   Rust checks, Linux build
#            sccache
#   android  + JDK 17, Android SDK/NDK, Rust targets Android build
# Built and pushed by images.yml (.github/scripts/build-images.sh) under the
# tag scripts/ci/image-tag.sh computes from this file, apt-packages.sh and
# the [tools] of mise.toml.

FROM docker.io/library/ubuntu:22.04 AS node
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ENV DEBIAN_FRONTEND=noninteractive \
    MISE_DATA_DIR=/opt/mise \
    MISE_CACHE_DIR=/opt/mise-cache \
    MISE_YES=1 \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo
ENV PATH=/opt/mise/shims:/opt/cargo/bin:$PATH
COPY scripts/ci/apt-packages.sh /opt/ci/apt-packages.sh
RUN source /opt/ci/apt-packages.sh \
 && apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends "${APT_CORE[@]}" \
 && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://mise.run | MISE_INSTALL_PATH=/usr/local/bin/mise sh
COPY mise.toml /opt/ci/mise.toml
RUN cd /opt/ci && mise trust mise.toml && mise install node pnpm \
 && rm -rf /opt/mise-cache && touch /etc/mwt-ci-image
COPY scripts/ci/install-chrome.sh /opt/ci/install-chrome.sh
RUN source /opt/ci/apt-packages.sh \
 && apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends "${APT_BROWSER[@]}" \
 && rm -rf /var/lib/apt/lists/* \
 && /opt/ci/install-chrome.sh /opt/chrome
ENV CHROMIUM=/opt/chrome/chrome-headless-shell-linux64/chrome-headless-shell

FROM node AS linux
ARG SCCACHE_VERSION=0.19.0
RUN source /opt/ci/apt-packages.sh \
 && apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends "${APT_BUILD[@]}" "${APT_GUI[@]}" \
 && rm -rf /var/lib/apt/lists/*
RUN cd /opt/ci && mise install rust \
 && rustup component add rustfmt clippy \
 && rustup component remove rust-docs \
 && rm -rf /opt/mise-cache /opt/cargo/registry
RUN curl -fsSL "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
  | tar -xz --strip-components=1 -C /usr/local/bin "sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache" \
 && sccache --version

FROM linux AS android
ARG ANDROID_PLATFORM=android-35
ARG ANDROID_BUILD_TOOLS=35.0.0
ARG ANDROID_NDK_VERSION=27.2.12479018
ENV ANDROID_HOME=/opt/android-sdk \
    ANDROID_SDK_ROOT=/opt/android-sdk \
    JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 \
    NDK_HOME=/opt/android-sdk/ndk/${ANDROID_NDK_VERSION} \
    ANDROID_NDK_HOME=/opt/android-sdk/ndk/${ANDROID_NDK_VERSION}
ENV PATH=/opt/android-sdk/build-tools/${ANDROID_BUILD_TOOLS}:/opt/android-sdk/platform-tools:$PATH
RUN source /opt/ci/apt-packages.sh \
 && apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends "${APT_ANDROID[@]}" \
 && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /opt/android-sdk/cmdline-tools \
 && curl -fsSL -o /tmp/tools.zip https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip \
 && unzip -q /tmp/tools.zip -d /tmp/tools \
 && mv /tmp/tools/cmdline-tools /opt/android-sdk/cmdline-tools/latest \
 && rm -rf /tmp/tools /tmp/tools.zip \
 && (yes || true) | /opt/android-sdk/cmdline-tools/latest/bin/sdkmanager --licenses >/dev/null \
 && /opt/android-sdk/cmdline-tools/latest/bin/sdkmanager --install "platform-tools" \
      "platforms;${ANDROID_PLATFORM}" "build-tools;${ANDROID_BUILD_TOOLS}" "ndk;${ANDROID_NDK_VERSION}" >/dev/null
RUN rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android \
 && touch /etc/mwt-ci-android
