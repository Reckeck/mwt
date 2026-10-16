# Local cross build for Windows (NSIS installer) from Linux, on top of the
# ci-linux image: makensis, the LLVM tools that understand MSVC targets
# (clang-cl is clang itself: the name picks the MSVC-style driver),
# cargo-xwin (fetches the Windows SDK and CRT on first use) and the Rust
# target. Ubuntu 22.04 ships NSIS 3.08, whose includes lack
# Win/RestartManager.nsh that the tauri installer needs: taken from 3.11.
# Only scripts/local/build-all.sh uses it; CI builds on Windows.
ARG BASE
FROM ${BASE}
RUN apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends nsis llvm lld clang zip \
 && rm -rf /var/lib/apt/lists/* \
 && llvm="$(dirname "$(ls /usr/lib/llvm-*/bin/clang | head -n1)")" \
 && ln -sf "$llvm/clang" /usr/local/bin/clang-cl \
 && for tool in lld-link llvm-lib llvm-rc llvm-dlltool; do \
      [ -e "/usr/bin/$tool" ] || ln -s "$llvm/$tool" "/usr/local/bin/$tool"; \
    done \
 && curl -fsSL -o /usr/share/nsis/Include/Win/RestartManager.nsh \
      https://raw.githubusercontent.com/kichik/nsis/v311/Include/Win/RestartManager.nsh \
 && rustup target add x86_64-pc-windows-msvc \
 && cargo install --locked cargo-xwin \
 && rm -rf /opt/cargo/registry
