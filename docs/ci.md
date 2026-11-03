# CI/CD (GitHub Actions) and platforms

For how to work with the `feature/*`, `hotfix/*`, `development` and `main`
branches and with releases, see [workflow.md](workflow.md). This page covers how
CI is built.

CI and releases live in GitHub Actions (`.github/workflows/`); the logic is in
`cargo xtask` (`crates/xtask`), the scripts `scripts/ci/*` and
`.github/scripts/*`, and the packaging manifests in `packaging/`. The
Rust/Node/pnpm versions come from `mise.toml` (on Linux through mise itself, on
Windows the script reads them from the file). GitLab (`Reckeck/trade-app`) is
only a mirror without CI.

## Workflows

| File | When it runs | What it does |
|---|---|---|
| `ci.yml` | PR into `development`/`main`, manually, `workflow_call` from `release.yml` | checks: Rust, frontend, layout/links in shards, Android, clippy for Windows/macOS, the `ci-ok` summary |
| `images.yml` | called from `ci.yml`, manually | builds and pushes the CI images to ghcr if the tag does not exist yet |
| `release.yml` | push to `development` (pre-release `X.Y.Z-N`) and `main` (release `X.Y.Z`), manually | version, checks, builds, packages, GitHub Release publication |
| `audit.yml` | PR, push to `development`/`main`, daily, manually | dependency audit (`cargo xtask audit`) |
| `dependabot-merge.yml` | Sunday, manually | merges green Dependabot updates |
| `cache-cleanup.yml` | PR closed, branch deleted | deletes the branch's caches |

Feature branches and hotfixes get only checks (through a PR); builds and
releases belong only to `development` and `main`, and only after the checks are
green. The `vX.Y.Z` tags are created by the `publish` job together with the
release. The workflows are checked with
`mise x actionlint@1.7.12 -- actionlint`.

## Speed

What makes CI fast and what must not be broken:

- **Ready-made images.** Jobs run in images from `scripts/ci/ci.Dockerfile`
  (ghcr.io, `ghcr.io/<owner>/<repository>/ci-<kind>:<tag>`): `ci-node`
  (mise + node/pnpm, frontend), `ci-linux` (+ Tauri libraries, Rust,
  sccache), `ci-android` (+ JDK 17, SDK/NDK, Rust targets). A job installs
  neither apt packages nor a toolchain: `setup-linux.sh` only attaches the
  caches. The image tag is a hash of the inputs (Dockerfile,
  `apt-packages.sh`, `install-chrome.sh`, `[tools]` from `mise.toml`); it is
  printed by `scripts/ci/image-tag.sh` and `mise run ci-image-tag`. The tag is
  written by hand nowhere: `images.yml` (`.github/scripts/build-images.sh`)
  builds the missing images itself, and a tag that is already pushed is not
  rebuilt (a manual run has `force`).
- **sccache.** Compiled crates live in the Actions cache (`.cache/sccache`, per
  lane: `ci.yml`, job `deps`, the key is `xtask lane-key`, which
  includes the image tag) and come back through `RUSTC_WRAPPER=sccache`. The
  lane's cache is written only by `deps` (and only when the key has none yet);
  the other jobs read it. The cache is visible to its own ref, the PR's base
  branch and `development`; `main` runs without caches (a release is built from
  scratch).
- **sccache locally.** `cargo xtask` (`mise run check`, `check-platforms`) sets
  `RUSTC_WRAPPER=sccache` for all cargo tasks on the host and starts the server
  before the graph; there is one cache per machine: `~/.cache/sccache/host`
  (16 GB, all working copies). The Android containers and `build:all` get
  `~/.cache/sccache/containers` (4 GB) as `/sccache`; the directories are
  separate because the keys do not overlap, and two servers in one directory
  would evict each other's entries. A `RUSTC_WRAPPER`/`SCCACHE_DIR` that is
  already set is not overridden; without sccache in PATH the build goes as
  before and prints a hint (`mise use -g sccache`); `XTASK_SCCACHE=0` turns it
  off everywhere. At the end of a run xtask prints `sccache: N requests, H hits,
  M misses`. sccache does not cache incremental workspace crates, only
  dependencies. Nothing changes in CI: `setup-linux.sh` takes `SCCACHE_DIR`
  from the environment, and by default it stays `.cache/sccache`.
- **Fast tests.** The vault KDF (Argon2id, 64 MiB x 3) runs on every save;
  `argon2`/`blake2` are built with `opt-level = 3` even in debug (`Cargo.toml`),
  otherwise `cargo test` took about 5.5 min instead of about 30 s. The KDF
  parameters did not change.
- **A new push cancels the old run** (`concurrency` with `cancel-in-progress`
  on all branches except `main`): a release from `main` is not interrupted.
- **Pre-releases without LTO.** The dev channel (pre-releases from
  `development`, manual and local builds) is built with
  `CARGO_PROFILE_RELEASE_LTO=off` and `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`:
  the final optimization runs in parallel instead of in a single thread. The
  variables are set by `cargo xtask build` itself according to the channel
  (`Channel::build_env`; they are forwarded into the container) and by
  `scripts/ci/build-windows.ps1` according to `RELEASE_CHANNEL`; a value set
  explicitly in the environment takes precedence. Releases from `main` (prod)
  are built with full LTO and a single codegen unit (`[profile.release]` in
  `Cargo.toml`), which gives the smallest and fastest binary.
- No debug info in the checks (`CARGO_PROFILE_DEV_DEBUG=0`).

## Releasing a version

Pre-releases are every push to `development`, a release is a merge into `main`;
the steps are in [workflow.md](workflow.md#release). The build version is
`cargo xtask version --channel dev|prod|none` (dev is `X.Y.Z-N`, where `N` is
the last published tag of the same `X.Y.Z` plus one; manual builds are
`X.Y.Z-<run number>`, prod is `X.Y.Z`); it gets into the build through
`tauri build --config` (`cargo xtask tauri-config`), together with the Android
`versionCode` = the run number (`GITHUB_RUN_NUMBER`).

### Release through xtask (GitHub)

The release service steps are moved into xtask commands
(`crates/xtask/src/publish/`); each one understands `--dry-run` (prints the
commands and does nothing on GitHub):

| Command | What it does |
|---|---|
| `cargo xtask publish --version V --dir D [--pre]` | `SHA256SUMS`, then `gh release create`; `--pre` for a pre-release `X.Y.Z-N` from `development` |
| `cargo xtask prune-prereleases [--keep N]` | keep the `N` latest pre-releases (by default `PRERELEASES_KEPT` or 10), delete the rest and their tags |
| `cargo xtask backmerge --version V` | a PR `main` into `development` after a release |
| `cargo xtask publish-aur --version V --dir D` | push `multi-wallet-tracker-bin` to AUR (`AUR_SSH_PRIVATE_KEY`) |

The token is `GH_TOKEN` (or `GITHUB_TOKEN`). They are called by
`.github/workflows/release.yml` (see the sections "Checks in ci.yml" and
"release.yml" below).

### Channels: the release and the dev app

Builds of `main` (`RELEASE_CHANNEL=prod`) and `development` (`dev`) are two
different apps: they install side by side, and each has its own data. The names
are set by `crates/xtask/src/channel.rs`; the overlay `cargo xtask tauri-config`
puts them into the build (`build-windows.ps1` repeats them, `mise run check-ci`
compares them):

| | prod (`main`, release) | dev (`development`, manual, local) |
|---|---|---|
| Name (`productName`) | Multi Wallet Tracker | Multi Wallet Tracker Dev |
| Binary, packages | `multi-wallet-tracker`, AUR `multi-wallet-tracker-bin` | `multi-wallet-tracker-dev`, pacman `multi-wallet-tracker-dev-bin` |
| Desktop (vault path) | `mwt.desktop` | `dev.mwt.desktop` |
| Android | `mwt.app` | `dev.mwt.app` |
| Flatpak | `mwt.app.MultiWalletTracker` | `dev.mwt.MultiWalletTracker` |

- `tauri.conf.json` / `tauri.android.conf.json` hold the dev values: this is how
  `mise run dev`, `mise run build` and builds without a channel (`none`) are
  built.
- The keyring key and the quick-add socket are tied to the build identifier
  (`crates/app/src/app_id.rs`), autostart to `productName`, so removing one app
  does not touch the other (`packaging/linux/postrm.sh`,
  `crates/app/windows/autostart-cleanup.wxs`).
- The TradeApp data migration (`legacy.rs`) is done only by dev: the release
  under `mwt.desktop` starts with a clean directory.
- The Flatpak manifest in the repository is dev; the release `build-flatpak.sh`
  renders it from that under its own ID.

### Overlay and channel names for the build scripts

The `scripts/ci/build-*.sh` scripts do not call xtask (there is no cargo in the
pacman and flatpak containers), so the channel names and the Tauri config reach
them from outside:

- `MWT_TAURI_CONFIG` is the path to the overlay (`tauri build --config`), written
  by `cargo xtask tauri-config` (by default `.cache/tauri-ci.json`; `cargo xtask
  build` writes its own file per platform, `.cache/tauri-ci-<platform>.json`);
- `MWT_APP_NAME`, `MWT_APP_BIN`, `MWT_APP_ID`, `MWT_APP_ANDROID_ID`,
  `MWT_APP_FLATPAK_ID`, `MWT_APP_ICONS` are the channel values from
  `Channel::env()` (`crates/xtask/src/channel.rs`).

## Checks in ci.yml

The checks run in `.github/workflows/ci.yml` (images in `images.yml`, cache
cleanup in `cache-cleanup.yml`), the release in `release.yml`. `ci.yml` runs on
a PR and is called from `release.yml` (`workflow_call`): a push to
`development`/`main` is checked by exactly that call; there is no separate run
of `ci.yml` on a push.

- `xtask` and `xtask-windows` compile xtask once per run (cache of
  `.cache/xtask-target`) and upload it as the artifacts `xtask-linux` /
  `xtask-windows` (not `xtask`: `release.yml` calls `ci.yml` in the same run and
  already uploads that name). Every job below downloads the binary into `bin/`
  and runs `bin/xtask ...` instead of `cargo xtask ...`; the cache helpers are
  hidden xtask commands (`lane-key`, `deps-cook`, `deps-prune`, `ci-env`,
  `install-chef`, in `crates/xtask/src/cache/`), the former `.github/scripts/*.sh`.
  The Linux binary is built in `ci-linux` (ubuntu 22.04, like `ci-node` and
  `ci-android`). Only the paused macOS job still uses `cargo xtask`. The
  Windows build in `release.yml` also compiles xtask for its cache steps: the
  `xtask` artifact of that workflow is a Linux binary. The lane keys changed
  once with this move (the key hashes were reimplemented), so every lane built
  its cache anew.
- `deps-node` installs `node_modules` once per key (`xtask lane-key node`: the
  lock file, manifests, `.npmrc`, image tag, node and pnpm versions) and saves
  it on a miss only, deleting the older node_modules caches of the ref;
  `check-frontend` and `frontend-layout` restore exactly that key with
  `fail-on-cache-miss` and never run `pnpm install`. They run no cargo, so they
  have no Rust lane cache and no sccache.
- `check-linux` (`bin/xtask check --exclude @frontend --exclude @platforms`) and
  `check-frontend` (`bin/xtask check @frontend --exclude layout --exclude links`) are two separate jobs in the
  `ci-linux` image: the Rust checks and the web part (lint, typecheck, vitest,
  static checks, e2e) run in parallel. A new task of `check` lands in
  `check-linux` unless it is in `@frontend` or `@platforms`; the groups are one
  table (`crates/xtask/src/checks/groups.rs`: `@static`, `@browser`, `@frontend`,
  `@rust`, `@platforms`) and a test fails for a task that is in none.
- `frontend-layout` runs the full `check-layout` and `check-links` (all widths,
  both themes) as a matrix of 6 shards: `bin/xtask check layout links
  --shard K/6 --jobs 1` (layout and links run in full unless `--quick`). The work is split into small units (a
  batch of about 5 theme x width combinations, a batch of about 10 overlays, the
  links page); shard K takes the K-th share by the weights from
  `tests/e2e/layout/weights.json` and `tests/e2e/links/weights.json` (seconds
  per unit, a greedy split "heaviest onto the lightest", deterministic). A unit
  missing from the file gets the default weight (the median), an extra key is
  ignored. The weights are updated by a full local run: `mise run check-layout
  -- --update-weights` (and `check-links`), and the result is committed; the
  timings of every run are in `.cache/check-<name>/timings.json`. Locally the
  same split between processes of one machine is `--jobs N` (one Vite, its own
  Chromium per process); a shard and `--jobs` add up. `fail-fast: false`: a
  failed shard does not stop the others; the logs and screenshots are the
  artifact `frontend-layout-<K>-logs`.
- macOS in CI is paused: the `clippy-macos` job is left in the file but is
  commented out in the matrix.
- `SCCACHE_BASEDIRS` (`xtask ci-env`) affects only C/C++; sccache
  does not apply it to rustc.

### Merging Dependabot updates (`dependabot-merge.yml`)

Dependabot opens PRs into `development` on Mondays (`.github/dependabot.yml`):
minor and patch updates of one ecosystem go in a single group PR, each major in
its own. Every Sunday at 06:00 Moscow time (`cron: 0 3 * * 0`)
`dependabot-merge.yml` merges (squash) those of them whose checks are all green
(including `ci-ok`), that have no conflict and whose update is not a major (for
`0.x` a major is a change of the second digit; a group PR without "from A to B"
means only the groups `cargo-minor`, `npm-minor`, `actions`). Majors and red,
running or conflicting PRs are left for manual review; after a neighbouring
merge Dependabot rebases a conflicting PR itself, and it is merged a week later.
A merge with the `GITHUB_TOKEN` token does not trigger push workflows, so after
merging the script starts `release.yml` (dev, publish) on `development` itself.
The list of merged and skipped PRs with the reason is in the run summary.
Manually: Actions → dependabot-merge → Run workflow; `dry_run` only shows what
would be merged. The logic is in `.github/scripts/dependabot-merge.sh`.

### Dependency audit (`audit.yml`)

`mise run audit` (`cargo xtask audit`) checks `Cargo.lock` (`cargo audit`, needs
`cargo install cargo-audit --locked --version 0.22.2`) and `pnpm-lock.yaml`
(`pnpm audit --audit-level high`). Vulnerabilities, `unsound` and `yanked` fail
for Rust, and `high`/`critical` for npm; `unmaintained` and minor npm findings
are only printed. The audit is not part of `mise run check`. The separate
workflow `audit.yml` runs on PRs into `development`/`main`, on pushes to them,
daily (cron) and manually; it is not part of `ci-ok`: a new finding in an
untouched dependency must not block other people's PRs.

An exception is an `[[ignore]]` entry in `audit-ignore.toml` in the root: `id`
(RUSTSEC or GHSA; it also matches an alias), `tool` (`cargo` or `pnpm`) and a
non-empty `reason` (why it is accepted and when it can be removed). An entry
that suppresses nothing any more is reported by the audit as a warning: it has
to be deleted.

### release.yml

A push to `development` is a pre-release `X.Y.Z-N` of the dev channel, a push to
`main` is a release `X.Y.Z` of the prod channel; a PR does not start a release.
The graph (`needs`):

```
version, images ──► frontend ──► build (linux | android | windows) ──► package (pacman | flatpak)
   │      │                    │                                                     │
   │      │                    └──► build-macos (paused)                             │
   │      └──► checks (ci.yml), in parallel with the builds ──► cancel-on-red (if red)
   └──────────────────────────────────────────────────────────► publish ◄───────────────┘
                                                                  ├──► aur       (prod only)
                                                                  └──► backmerge (prod only)
```

- `version` decides the channel and whether a release is needed
  (`.github/scripts/release-plan.sh`), builds `xtask` (the only Rust compiler in
  the run; the binary goes as an artifact to all the following jobs), computes
  `X.Y.Z[-N]` (`cargo xtask version`: fails if the tag `vX.Y.Z` already exists)
  and writes the Tauri overlay for Windows. The Android `versionCode` is
  `GITHUB_RUN_NUMBER` (only grows); the dev pre-release number `N` comes from
  the tags of the remote repository, not from the run number.
- `images` are the CI images (`images.yml`); `frontend`, the builds and `checks`
  depend on them. The builds do not wait for the checks: they run in parallel
  with them.
- `checks` is the whole `ci.yml`. `publish` requires `checks` to be `success`;
  red checks start `cancel-on-red` (`actions: write`, `gh run cancel`), and the
  remaining builds are cancelled.
- `frontend` runs `pnpm build` once and `dist/` goes as an artifact; the builds
  call `--prebuilt-frontend` (an overlay without `beforeBuildCommand`).
- `build` is one group with `fail-fast: true`: the first failed build cancels
  the others (restart with "Re-run failed jobs"). linux and android are
  `xtask build <platform>` on a runner with the `ci-linux`/`ci-android` images
  from ghcr (docker), windows is `scripts/ci/build-windows.ps1` on
  `windows-latest` (the MSI is built by WiX). Pre-releases go without LTO on 16
  codegen units, a release with the LTO from `[profile.release]`. The sccache
  cache of the container builds is handed back to the runner user (`chown`)
  before saving: docker writes it as root. The cache key (`xtask lane-key`)
  includes the channel; on Windows `SCCACHE_IDLE_TIMEOUT=0` keeps the server
  alive through the LTO tail.
- `package` is pacman and Flatpak from the Linux tarball (`xtask build
  pacman|flatpak`, an archlinux container, Flatpak `--privileged`).
- `publish` is the only job with `contents: write`: a check of the file set
  (`.github/scripts/check-release-files.sh`), `SHA256SUMS`, the tag and the
  release (`cargo xtask publish [--pre]`), then `prune-prereleases` (keep 10; an
  error does not fail the run).
- `aur` (prod) is `publish-aur` in an archlinux container; without the secret
  `AUR_SSH_PRIVATE_KEY` the step is skipped with a warning.
- `backmerge` (prod, `pull-requests: write`) is a PR `main` → `development`; the
  token is `BACKMERGE_TOKEN` (if present), otherwise `GITHUB_TOKEN` (the setting
  "Allow GitHub Actions to create and approve pull requests" is needed); an
  error does not fail the run.
- `build-macos` is written but turned off (`if: false`, like `clippy-macos`): to
  turn it on, remove `if: false`, add it to `needs` and to the `publish`
  condition, and pass `macos` as the third argument of `check-release-files.sh`.

The release files (`<v>` is the version, decision 12 of the plan):

| Platform | Files |
|---|---|
| Linux | `multi-wallet-tracker_<v>_amd64.deb`, `multi-wallet-tracker-<v>-1.x86_64.rpm`, `multi-wallet-tracker-<v>-x86_64.AppImage`, `multi-wallet-tracker-<v>-linux-x86_64.tar.gz` (the AUR source) |
| Arch / Flatpak | `*.pkg.tar.zst` (pacman), `multi-wallet-tracker-<v>-x86_64.flatpak`; in AUR, `multi-wallet-tracker-bin` (prod) |
| Android | `multi-wallet-tracker-<v>-android-universal.apk` and `.aab` |
| Windows | `multi-wallet-tracker-<v>-x64-setup.exe` (NSIS), `multi-wallet-tracker-<v>-x64-<language>.msi`, `multi-wallet-tracker-<v>-windows-x64-portable.zip` |
| macOS (paused) | `multi-wallet-tracker-<v>-macos-universal.dmg`, `.app.tar.gz` |
| All | `SHA256SUMS` |

**Secrets** (Settings → Secrets and variables → Actions):

| Secret | Purpose | Without it |
|---|---|---|
| `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` | signing the APK/AAB; the previous keystore, otherwise the installed app will not update | on a release the Android build fails; without a release the APK is signed with a test key (`*-testkey`), and such files are not published |
| `ANDROID_KEY_PASSWORD` | the key password, if it differs from the keystore password | the keystore password is used |
| `AUR_SSH_PRIVATE_KEY` | push to AUR | the step is skipped with a warning |
| `BACKMERGE_TOKEN` | a PAT/app token for the PR `main` → `development` (it starts the checks) | the PR is made by `GITHUB_TOKEN` |
| `APPLE_*` | macOS signing (when it is turned on) | ad hoc signing |
| `TAURI_SIGNING_PRIVATE_KEY` | signing updates: `latest.json` + `.sig` (decision 16) | not used: the app has no update plugin yet |

The keystore is not written to the runner's disk: `ANDROID_KEYSTORE_BASE64`
reaches the container as an environment variable (GitHub masks the value in the
log), `sign-android.sh` lays it out into a temporary file inside the container,
and that file disappears together with the container.

**Manual run** (Actions → release → Run workflow, or
`gh workflow run release.yml --ref <branch> -f channel=dev -f publish=false`):
choose the branch, `channel` (`dev` or `prod`) and `publish`. With
`publish: false` (the default) `version`, the checks and all the builds run, the
artifacts stay in the run (`bundle-*`, 7 days), `publish`, `aur` and `backmerge`
are skipped, and no tag or release is created. A manual release is allowed only
as `dev` on `development` or `prod` on `main`. `dev` without a release gets the
version `X.Y.Z-N` (the same count by tags), `prod` is treated like a release
(it fails if `vX.Y.Z` is already released or CHANGELOG has no such section).

Cache: the checks inside `checks` read and write their own lanes, as in a PR;
the builds have no `deps` lanes of their own yet, so release builds run without
a cache of compiled crates (on `main` this is intended, decision 26).

## Runners

- The checks and Linux builds run on `ubuntu-latest` with docker; the builds run
  in the `ci-linux`/`ci-android` images from ghcr. Flatpak is built in a
  container with `--privileged` (bubblewrap).
- Windows is `windows-latest` (`scripts/ci/build-windows.ps1`; MSVC Build Tools
  are installed through choco if they are missing; the MSI is built by WiX).
- macOS in CI is paused (`clippy-macos`, `build-macos` are turned off).
- The Android build is long (release + LTO, three architectures,
  `ANDROID_TARGETS`): a 2 h timeout.
- `images.yml` needs docker and `packages: write` on `GITHUB_TOKEN`.

## Secrets (Settings → Secrets and variables → Actions)

The secrets that the workflows read (besides the automatic `GITHUB_TOKEN`):

| Secret | Needed for | Note |
|---|---|---|
| `ANDROID_KEYSTORE_BASE64` | release signing of the APK/AAB | `base64 -w0 release.keystore` |
| `ANDROID_KEYSTORE_PASSWORD` | the same | |
| `ANDROID_KEY_ALIAS` | the same | |
| `ANDROID_KEY_PASSWORD` | the same | by default = the keystore password |
| `AUR_SSH_PRIVATE_KEY` | `aur` (`publish-aur`) | a key added to the AUR account |
| `BACKMERGE_TOKEN` | `backmerge` | a PAT/app token; otherwise `GITHUB_TOKEN` |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | macOS signing (the job is turned off) | base64 `.p12` Developer ID; without them, ad hoc |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | macOS notarization (the job is turned off) | an Apple ID app password |

For the details of "without the secret", see the secrets table in the
"release.yml" section.

Without a key the Android APK is signed with a throwaway test key and gets the
suffix `-testkey`: it installs for testing, but does not update the previous
build and is not suitable for publication. The key is created once and kept
outside the repository:

```bash
keytool -genkeypair -v -keystore release.keystore -alias mwt \
  -keyalg RSA -keysize 4096 -validity 10000
```

Losing the key means being unable to update the app for users.

## Platforms: specifics

- **Icons.** The sources are SVGs in `crates/app/icons/src/`; `mise run icons`
  (`scripts/icons.sh`, needs `rsvg-convert` and `magick`) builds PNG/ICO/ICNS
  for both channels into `crates/app/icons/prod/` and `dev/` and
  `public/favicon.svg`; the PNGs are committed, CI draws nothing.
  `tauri.conf.json` points to `dev/`, the overlay `cargo xtask tauri-config` /
  `build-windows.ps1` to the channel's folder (`Channel::icons` in
  `channel.rs`). Sizes of 32 px and smaller come from `*-small.svg` (without the
  back edges of the cube); dev has an orange DEV stripe.

- **Android.** The Gradle project (`crates/app/gen/android`) is generated in CI
  (`tauri android init`) and is not committed; the adaptive icon (background,
  foreground, monochrome layer) is made by `tauri icon` from the manifest
  `crates/app/icons/<channel>/android/manifest.json`. The identifier is
  `mwt.app` in the release and `dev.mwt.app` in dev
  (`crates/app/tauri.android.conf.json`), the desktop one is `mwt.desktop` /
  `dev.mwt.desktop` (the vault path depends on it); see "Channels". The tray,
  autostart, the global hotkey, saving the window geometry and the system
  keyring exist only on desktop (`cfg(desktop)`): on Android autostart is always
  off, a hotkey cannot be set (`UNSUPPORTED`), and the reminders of a locked
  vault live only in memory. The autostart and hotkey items in the settings are
  not hidden on the phone yet: toggling them there returns an error. Locally:
  `pnpm tauri android init` → `pnpm tauri android dev` (needs the Android SDK,
  NDK and JDK 17, and the variables `ANDROID_HOME`, `NDK_HOME`).
- **Flatpak.** The ID is `mwt.app.MultiWalletTracker` in the release,
  `dev.mwt.MultiWalletTracker` in dev (Flatpak does not like an ID ending in
  `.desktop`, and it needs at least 3 parts). The data is in
  `~/.var/app/<ID>/`. Autostart does not work from the sandbox. Flathub needs a
  separate PR to flathub with this manifest.
- **AUR / pacman.** The package `multi-wallet-tracker-bin` installs a ready
  binary from GitHub Releases, so the repository has to be public. The template
  is `packaging/aur/PKGBUILD.in`, the render is `packaging/aur/render.sh`.
- **macOS.** A universal build (`--target universal-apple-darwin`),
  `scripts/ci/build-macos.sh`. Without a Developer ID certificate the app is
  signed ad hoc and not notarized: on first launch Gatekeeper asks for
  confirmation (right click → "Open", or
  `xattr -cr "/Applications/Multi Wallet Tracker.app"`). Tauri cannot build for
  macOS on another OS. Autostart is a LaunchAgent in `~/Library/LaunchAgents`;
  when the `.app` is moved to the trash it stays (macOS has no uninstall hook),
  but there is nothing left to launch: turn autostart off in the settings before
  uninstalling.
- **Autostart after uninstall.** The autostart entry lives in the user profile,
  so uninstalling removes it separately: deb/rpm through
  `packaging/linux/postrm.sh`, pacman/AUR through `post_remove` in
  `packaging/aur/multi-wallet-tracker-bin.install` (they delete the entries that
  launch `/usr/bin/multi-wallet-tracker`, for all users), NSIS through the
  built-in tauri uninstaller step, MSI through the fragment
  `crates/app/windows/autostart-cleanup.wxs`. On Linux the entry also carries
  `TryExec=<path to the program>` (`crates/app/src/autostart.rs`): the session
  skips it if the file is missing, so an AppImage deleted by hand does not start
  either. An update does not touch the entry.
- **Windows.** The portable `.exe` and the installers require the WebView2
  Runtime (present in Windows 10/11; NSIS downloads it if it is missing). There
  is no code signing: SmartScreen will warn.

## Locally

### Compiling for all platforms: `mise run check-platforms`

A quick check without bundles (minutes, not an hour): `cargo clippy -D warnings`
for linux, windows (`cargo xwin`, `x86_64-pc-windows-msvc`), android
(`aarch64-linux-android`, the host's NDK, otherwise the `ci-android` image
through podman with the same volumes as `build:all`) and macos (only on a Mac or
through `MACOS_BUILD_HOST`). It is mandatory after the last edit, before
merging and committing (AGENTS.md, "Building on all platforms"). The logs are
`.cache/xtask/<task>.log` (`clippy` = linux, `platform-windows.log` …); it is
started by `cargo xtask check clippy @platforms` (the same tasks are part of `mise run
check`); `SKIP` with a reason means the machine cannot check that platform,
`FAIL` means the code does not build.

```bash
mise run check-platforms                    # everything
mise run check-platforms -- windows android # selectively
```

### All platforms: `mise run build:all`

`cargo xtask build` builds the bundles with the same code as CI
(`scripts/ci/build-*.sh`), in the same images; the machine needs only podman (or
docker: `CONTAINER_ENGINE=docker`). The channel is `--channel dev` (the default)
or `prod` (see "Channels"). The result goes to `.cache/build-all/builds/<channel>/`,
the log of each task to `.cache/xtask/build-<platform>.log`, and at the end
there is an OK/SKIP/FAIL/CANCELLED table with times and a list of files. The
graph and behaviour are the same as for `mise run check`: the first error stops
the other builds (the containers are stopped), a second identical run joins the
first, a different command waits for it to finish.

```bash
mise run build:all                         # everything
mise run build:all -- linux android        # selectively
mise run build:all -- --version 0.2.0-1 windows
mise run build:all -- --channel prod       # the release app (mwt.*)
mise run build:all -- --heavy 1            # one heavy build at a time (2 by default)
mise run build:all -- --list linux pacman  # the task graph without running
```

The graph: `build:frontend` (`pnpm build` on the host, once, into `dist/`) →
`android`, `windows`, `linux` (heavy, no more than `--heavy` at a time) →
`pacman`, `flatpak` (after `linux`); `macos` waits for nobody. The per-platform
`tauri build` runs do not run the frontend again: the overlay `cargo xtask
tauri-config --prebuilt-frontend` has an empty `beforeBuildCommand` (CI does
not pass the flag). `pacman` and `flatpak` without `linux` in the list take the
tarball from the result folder, and if it is not there, it is an error before
the build starts ("build it first").

| Platform | Where it is built | What you get |
|---|---|---|
| `linux` | the `ci-linux` image (`--native` on the host) | `.deb`, `.rpm`, `.AppImage`, `tar.gz` |
| `pacman` | `archlinux:base-devel`, after `linux` | `.pkg.tar.zst` |
| `flatpak` | `archlinux:base-devel` `--privileged`, after `linux` | `.flatpak` |
| `android` | the `ci-android` image | APK and AAB; without `ANDROID_KEYSTORE_*` a test key, suffix `-testkey` |
| `windows` | `ci-linux` + `cargo-xwin` (`scripts/local/windows-cross.Dockerfile`) | NSIS `-setup.exe`, portable `.zip`; no MSI: WiX works only on Windows, CI makes it |
| `macos` | on a Mac immediately; from Linux on `MACOS_BUILD_HOST=user@mac` over ssh (rsync there and back, the Mac builds the frontend itself) | `.dmg`, `.app.tar.gz`; without a Mac, SKIP |

- The images come from ghcr.io (`ghcr.io/reckeck/mwt/ci-<kind>:<tag>`, the tag is
  `mise run ci-image-tag`; another registry is `MWT_CI_REGISTRY`), and if they
  are not there, they are built from `scripts/ci/ci.Dockerfile`.
- The container build directories are volumes `mwt-build-<hash of the copy
  path>-*` (separate for each copy: cargo compares file times, and foreign
  artifacts would look fresh; `.rust-target`, `node_modules`, `/root`): the
  host's `.rust-target` and `node_modules` are not touched, and a repeated build
  is incremental. To start from scratch:
  `podman volume rm $(podman volume ls -q | grep mwt-build-)`.
- The builds run side by side, so each has its own tauri overlay
  (`.cache/tauri-ci-<platform>.json`, the variable `TAURI_CI_CONFIG`), and
  Android works on a private copy of `crates/app/icons`
  (`MWT_ICONS_ISOLATED=1`): `tauri icon` rewrites the icons while linux and
  windows are reading them.
- The default version is from `tauri.conf.json` (`cargo xtask version`).
- The first `windows` takes longer: an image with `cargo-xwin` is built, and it
  downloads the Windows SDK (the cache is in the volume
  `mwt-build-<copy>-windows-home`).

### In parts

```bash
mise run build                                       # deb/rpm/AppImage
scripts/ci/collect-linux.sh "$(cargo xtask version)" release
packaging/aur/render.sh 0.1.0 release/multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz > PKGBUILD
```
