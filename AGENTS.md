# AGENTS.md — instructions for AI developers

## Language

- **The repository is in English**: code (names, comments), documentation,
  commit messages, skills, `AGENTS.md`/`CLAUDE.md`, script and tool output.
- **The UI is Russian + English**: user-facing texts live in the locales
  `src/shared/config/i18n/locales/{ru,en}.json`; the key sets must match.
- **The agent talks to the user in Russian** in chat (reports, questions,
  plans shown to the user); everything that is written into the repository is English.

## Start of every session

As the first action in a new chat, load the `workflow` skill. Delegate analysis,
search and changes to subagents; the main session plans, writes the brief,
verifies the result and reports (details: the `workflow` skill, the
"Subtasks" section below).

## Commands (mise)

Always use `mise run …` (the `mise.toml` task file), do not hardcode versions:

| Command | What it does |
|---|---|
| `mise run test` | `cargo test --workspace` |
| `mise run typecheck` | `vue-tsc --noEmit` |
| `mise run test-frontend` | `vitest run --project unit`: frontend unit tests (`src/**/*.test.ts`, happy-dom, `call()`/`onEvent` and i18n mocked in `src/test/setup.ts`) |
| `mise run lint` | `pnpm exec biome check .` (lint + format, Biome) |
| `mise run format` | `pnpm exec biome check --write .` — Biome autofix |
| `mise run clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `mise run gen-types` | regenerate `src/shared/api/ipc.gen.d.ts` from Rust (ts-rs) |
| `mise run test-e2e` | `vitest run --project e2e`: end-to-end scenarios (`tests/e2e/flows/*.e2e.ts`) — the app on the mock in headless Chromium; Vite and the browser start once (`tests/e2e/harness/`); the snapshot of a failed test is in `.cache/e2e/` |
| `mise run check-onboarding` | every screen has a tour, the onboarding texts and "What's new" exist in both locales (rule 12) |
| `mise run check-ui` | shared components instead of custom markup, tokens instead of raw colors, `<Money>` instead of a `formatMoney` string in a template and in slice lib/model (`money-text`, `money-lib`), `meta` with an outcome on every mutation, file size limit (rule 15); `dvh` instead of `vh`, `*-safe` on `fixed` at an edge, wide widths only with a prefix, hover reveal only with `pointer-fine:` (rule 16); listeners without cleanup, mounting on hover, swapped buttons, queries without `placeholderData`, `RouterView :key`, `backdrop-blur`, fixed grid tracks, `z-*`/size scales, `--container-*` in `@theme` (rule 17) |
| `mise run check-forms` | on forms the save button is not disabled by field validation, fields are validated on save (rule 13); account, asset and currency are chosen with the ready-made components (rule 14) |
| `mise run changelog` | update the commit list in `## [Unreleased]` of CHANGELOG.md (git-cliff) |
| `mise run bump` | next version (`auto`/`patch`/`minor`/`major`/`X.Y.Z`) in all files + a CHANGELOG section |
| `mise run check-platforms` | compilation (clippy) for linux, windows, android, macos; `SKIP` — the machine cannot check that platform (the "Building on all platforms" rule); runs `cargo xtask check clippy @platforms`; the same tasks are part of `mise run check`; selected platforms — `mise run check-platforms -- windows android` |
| `mise run check-ci` | CI scripts: `*.ps1` ASCII only, shell scripts parse (`scripts/ci/lint-ci.sh`) |
| `mise run audit` | dependency audit of Rust (cargo-audit) and npm (`pnpm audit`); exceptions with reasons are in `audit-ignore.toml`; not part of `check`, in CI it is `audit.yml` |
| `mise run check-layout` | the app on the mock backend in headless Chromium at 1920…720 and 390/360, dark and light theme: no overflow and no sideways scroll, hover and loading shift nothing, a dialog opens in place, listeners do not pile up, "Back" closes a layer, touch targets ≥ 40 px, neighbouring blocks of one container do not overlap (rule 17); `--quick` — 1280 and 390, `--report` — without failing, `--jobs N` — N processes (default min(4, cores/2)), `--shard K/N` — the K-th share of N, `--update-weights` — weights from a measurement; screenshots in `.cache/check-layout/` |
| `mise run check` | `cargo xtask check --quick`: lint + typecheck + test-frontend + static checks (vitest `checks`: onboarding/routes/forms/ui/fsd) + ci + fmt + clippy + tests + gen-types + platforms in parallel, then one at a time test-e2e, layout and links (`--quick`); fail-fast — the first error stops the rest; logs in `.cache/xtask/<task>.log`; task names and groups (`@static @browser @frontend @rust @platforms`, `checks/groups.rs`) select, `--exclude` removes, `--keep-going` does not stop on the first error, `--list` / `--dry-run` show the graph / the commands, `--output auto\|tty\|plain\|github` and `-v`/`-q` shape the output (GitHub: a group per task, `::error` at file:line, a Job Summary); plain `cargo xtask check` runs layout and links in full; a second run in the same working copy attaches to the running one |
| `mise run build` | tests + typecheck + `pnpm tauri build` (deb/rpm/AppImage) |
| `mise run build:all` | `cargo xtask build`: bundles for every platform (linux, pacman, flatpak, android, windows without MSI, macos on a Mac) in CI images via podman, frontend built once, fail-fast, `--channel prod`, into `.cache/build-all/builds/<channel>/` (`docs/ci.md`, "Locally") |
| `mise run dev` | Tauri dev (`pnpm tauri dev`; the Tauri v2 CLI finds `crates/app/tauri.conf.json` from the workspace root by itself) |
| `pnpm dev` | Vite frontend without Tauri |

Also useful directly: `cargo build --workspace`, `cargo test -p trade-core`.

## Structure

```
Cargo.toml            # cargo workspace: crates/core + crates/app, edition 2024
crates/core/          # all business logic, WITHOUT Tauri
  src/lib.rs          # re-export: Amount, Error/Result, model::*, VaultFile, SnapshotFormat
  src/error.rs        # Error enum: Crypto/Secret/NotFound/Unsupported/…, method .code()
  src/amount.rs       # Amount (Decimal newtype), string serialization; custom ts-rs TS impl
  src/model/          # domain: chain.rs, store.rs, transaction.rs, vault.rs (VaultDoc, Settings…)
  src/appconfig.rs    # app settings OUTSIDE the vault: theme, locale, base_currency, favorites
  src/crypto/         # header.rs (55 bytes, AAD) + mod.rs (seal/open/atomic_write)
  src/storage/        # SnapshotFormat (Json/Bin + for_profile()), VaultFile::create/open/save
  src/vault_io/       # vault file on any OS: local (atomic_write, lock, .bak), path|URI, import, mirror (docs/storage-platforms.md)
  src/pricing/        # PricingService: fetch_prices/fetch_fx (pure, no doc mutation)
  src/service.rs      # UnlockedVault: mutations, holdings, portfolio, history, PnL
  src/market/         # Binance OHLCV candles for charts (used to live in sim/, the lab was removed)
  src/ipc_contract.rs # DTOs for commands (IPC inputs/outputs)
  src/bin/gen_types.rs# generator of src/shared/api/ipc.gen.d.ts (ts-rs export_all_to + merge)
crates/plugin-saf/    # Android: document picker with a persistent grant (Kotlin), android target only
crates/app/           # Tauri shell: builder, commands/*, tauri.conf.json, capabilities/
  src/commands/       # auth, stores, transactions, categories, contacts, planning, analytics, pricing, portfolio, calc, settings, exchange
  src/platform_fs/    # the ONLY place with file dialogs, content:// and the vault directory: desktop/mobile, copy in an external file
  src/scheduler.rs    # background thread: Bybit import, automations, alerts, reminders
  src/lib.rs          # tauri::Builder + generate_handler![…] (list of ALL commands), AppState (vault: Mutex<Option<UnlockedVault>>, vault_path)
src/                  # frontend (Vue 3 + Pinia + TanStack Query + vue-i18n + ECharts), FSD v2.1
  main.ts             # bootstrap: createApp + router/pinia/vue-query/i18n
  app/                # top layer: entry point and infrastructure
    providers/        # router/, query/ — infrastructure providers
    styles.css        # global styles (Tailwind v4, base)
  pages/              # screens, one per route of app/providers/router; composition ONLY
    dashboard/ stores/ wallet/ wallet-settings/ transactions/ planning/ analytics/ directory/ unlock/ app-shell/
  widgets/            # composite page blocks (store-header, dashboard-total, sidebar-wallets, …)
  features/           # user actions (vault-unlock, store-create, price-refresh, …)
  entities/           # business entities: store/, transaction/, asset/, currency/
  shared/             # bottom layer, imports nothing above itself
    ui/               # shadcn-vue/Reka primitives: a FOLDER per component + index.ts
    lib/              # utilities: format, echarts, theme, locale, transactions, vault-error…
    api/ipc.ts        # the ONLY place that calls Tauri (call/onEvent); api/mock/ — mock for pnpm dev, loaded lazily
    api/ipc.gen.d.ts  # AUTO-GENERATED, regenerated by mise run gen-types
    store/            # Pinia: vault.ts, settings.ts (global state)
    config/i18n/locales/  # ru.json + en.json, flat dotted keys; the key sets must match
.agents/skills/       # agent skills, ALL here (see the table below)
mise.toml             # tools + env + tasks
.github/workflows/    # CI/CD (GitHub Actions): checks, images, Linux/Windows/Android builds, Flatpak/pacman, release (docs/ci.md)
scripts/ci/           # CI job logic (bootstrap, bundle collection, Android signing, release, AUR)
crates/xtask/         # cargo xtask: the gate (check with task groups) and bundles (build)
packaging/            # flatpak/ (manifest + metainfo), aur/ (PKGBUILD.in + render.sh), linux/ (postrm deb/rpm)
docs/                 # architecture, data-model, ipc, security, roadmap, ui-design, json-editor, ci
```

## Code rules

1. **Core does not depend on Tauri.** Everything that can be covered by unit tests
   (crypto, model, service) is tested in core. Commands in `crates/app` are
   only a thin IPC wrapper.
2. **Money — only `Amount` (Decimal).** Do not use `f64` in domain
   quantities and prices. `f64` is allowed only in `market/` — OHLCV candles
   for charts, not money.
3. **Async Tauri commands.** Never hold a
   `MutexGuard<Option<UnlockedVault>>` across `.await` (the future is not `Send`).
   First copy the data you need, release the guard, then await, then take it
   again. Example: `refresh_all` in `commands/pricing.rs`.
4. **Network is separate from cache.** `PricingService::fetch_prices/fetch_fx` are
   pure (they do not touch `doc`), `apply_prices/apply_fx` apply the result.
5. **Vault file schema: compatibility or migration.** Existing vaults
   must open and keep their data after an update. Add an additive field
   with `#[serde(default)]`: JSON and MessagePack (release since v2) store
   field names, no bump needed. Do not rename, delete or change the meaning of
   a stored field or enum variant. If a change is incompatible — bump
   `VAULT_FORMAT_VERSION` **together with a migration**: freeze the current schema in
   `storage::vN`, add a step and tests for the JSON and binary formats (the
   user decided so on 2026-10-11; a bump without a migration is not allowed). The old bincode
   (`FMT_BIN`) is read-only. Details: the `vault-crypto` skill.
6. **IPC contract.** Keep `#[derive(TS)]` on all types from `ipc_contract.rs`
   and on the domain structs that reach commands. After a change, regenerate:
   `mise run gen-types`. Do not edit `src/shared/api/ipc.gen.d.ts` by hand.
   The `.gen.d.ts` name is deliberate: TypeScript does not include
   `foo.d.ts` in the program if `foo.ts` lies next to it (the name base is "shadowed"), so
   the file is called `ipc.gen.d.ts` next to `ipc.ts`.
7. **Edition 2024**: `gen`/`await` are reserved — no `rng.gen()`
   (use `rand::random::<f64>()`), no method/variable names `gen`.
8. **Crypto test vector**: wrong password/tampered file must give
   `Error::Secret`, never panic.
9. **FSD layer boundaries.** `app → pages → widgets → features → entities → shared`.
   A layer imports only from itself and from those listed below it; `shared` imports
   nothing from the other layers. Neighbouring slices of one layer
   import only through the public API (the slice's `index.ts`). A boundary violation
   is not a style issue but a reason to move the shared part into `shared`.
10. **Feature = user action, widget = section composite, entity =
    entity representation.** If a block only shows an entity — it is
    `entities/<name>/ui`; if it assembles a page section from several
    entities/features — `widgets`; if it performs a user operation — `features`.
    A page in `pages` is a thin composition, without domain logic of its own.
11. **A UI component is a folder with `index.ts`.** In `shared/ui` and `entities/*/ui`
    every component lives in its own directory, and `index.ts` **must**
    export: named components (no outside `default` export),
    prop types and constants (variants, sizes, option lists). Types and
    constants needed outside live in `*.types.ts` / `*.constants.ts` inside
    the folder — they cannot be exported from `<script setup>`.

12. **Onboarding is part of the feature, not a later follow-up.** A
    user-visible new or changed function (screen, dialog, action, field, behavior change)
    ships with a tour (`tours.ts`, `data-tour`), hints in place,
    an empty state with a first step, a "What's new" entry and texts in both
    locales; old steps and hints are updated. An internal change — the line
    `Onboarding: n/a — <reason>` in the commit. Check: `mise run
    check-onboarding`. Details: the `onboarding` skill.

13. **Forms: required fields are marked, errors go under the field on save.**
    A required field has a red `*`; the save button is **active** (only a save in progress
    or a backend refusal shown nearby blocks it);
    pressing it with an invalid field shows the message under it and sets focus —
    `useFormValidation` + the `shared/lib/form` validators; what the backend would reject by a
    known rule, the form checks itself. Check: `mise run
    check-forms`. Details: `ui-design-system/references/forms.md`.

14. **Account, asset and currency are chosen only with the ready-made components:**
    `StoreNodeSelect` / `AccountRefPicker`, `AssetInput` / `AssetSelect`,
    `CurrencyPicker` — in forms, filters, settings, node fields. If something is missing,
    add a prop to the component, not a list next to it. Check: `mise run
    check-forms`. Details: `ui-design-system/references/forms.md`.

15. **The interface is built from shared components and answers every
    action.** Table, empty state, loading/error, form dialog,
    page header, amount and P&L are shadcn-vue components and shared components of
    `shared/ui` (catalog — the `ui-design-system` skill), not custom markup;
    a repeated block is extracted into a shared component. Every mutation
    describes its outcome in `meta` (`success`/`error`, `undo` for the reversible;
    `silent` — with a reason), notifications are shown by `MutationCache`. Colors are
    tokens (`text-success`, `text-destructive`…), a file is one semantic
    part (`.vue` up to 250 lines, `.ts` up to 300). Check: `mise run
    check-ui`; old debts are in `scripts/check-ui.baseline.json`, they must not
    get worse.

16. **The interface works on a phone.** The boundary is `md` (768 px, `useIsPhone()` in the
    frontend); status bar, notch and gesture bar insets are
    only `*-safe` / `var(--safe-*)`; height is `dvh` or `h-full`, not `vh`;
    not a single action only by keyboard, hover or right click
    (on touch — the "⋯" `ActionMenu`); a dialog on a phone is a bottom sheet;
    the system "Back" closes the top layer (`shared/lib/back-stack`):
    a custom overlay outside Reka registers with `pushBackLayer`. Touch targets
    (≥ 40 px) are set in the variants of shared components, not in screen files;
    anything hidden until hover — only with `pointer-fine:` (`pointer-fine:opacity-0`).
    Check: `mise run check-ui`, `mise run check-layout` and by eye at 390
    and 360 px with inset emulation. Details: `ui-design-system/references/mobile.md`.

17. **The interface does not move by itself.** Hover, focus, press, loading and
    data refresh, the appearance of a scrollbar do not change the size or place of
    neighbours: the space for row buttons is reserved, a control is not
    swapped for another (`v-if`/`v-else`), a spinner does not widen a button,
    a scroller has `scrollbar-gutter: stable`. A query with a changing key
    keeps the previous data (`placeholderData: keepPreviousData`), a skeleton has the
    height of the content. A subscription to a backend event is only
    `useBackendEvent`, a timer is cleared in `onScopeDispose`. An overlay
    animation does not touch `translate`/`transform` of a positioned element;
    `backdrop-filter` is forbidden (WebKitGTK freezes on it). Check: `mise
    run check-ui` (statically) and `mise run check-layout` (the app in
    headless Chromium: overflow, shift on hover and loading, listener
    leaks, "Back", touch targets). Details:
    `ui-design-system/references/stable-layout.md`.

## Conventions

- **FSD layers.** The structure and import rules are described in the "Code
  rules" section (items 9–11). In short: `app/providers` (router, query),
  `pages` (composition), `widgets` (page blocks), `features` (actions),
  `entities` (entities), `shared` (ui, lib, api, store, config).

- Code in English; comments are not mandatory, but where context is needed —
  write it.
- Rust formatting and lint: `cargo fmt` + `mise run clippy`
  (`cargo clippy --workspace --all-targets -- -D warnings`, profile-based, CI-style).
- Frontend formatting and lint: Biome (`biome.json`) — `mise run lint`
  checks, `mise run format` fixes. Settings match the current code style:
  single quotes, no semicolons, 2 spaces, width 120.
  There is no ESLint in the project anymore.
- **No single-letter names.** Rust: the lint `clippy::min_ident_chars`
  (threshold 1, empty exception list — `clippy.toml` + `[workspace.lints]`),
  so `i`, `j`, `x` are forbidden too; mark an unused variable
  with a `_` prefix. Frontend: `style/useNamingConvention` in `biome.json`
  requires at least 2 characters (regex `t|.{2,}`) for variables and parameters;
  the only exception is `t` from `const { t } = useI18n()` (destructuring
  is not checked by the rule anyway, and the name `t` is allowed explicitly).
- No secrets in the repository; `.gitignore` ignores `data/`, `*.wallet`,
  `.rust-target/`, `node_modules/`, `dist/`, `src-tauri/target/`.

## CI/CD (GitHub Actions)

CI and releases are workflows in `.github/workflows/` (`ci.yml` — checks on PRs and
via `workflow_call` from `release.yml`; `images.yml` — CI images in ghcr;
`release.yml` — pre-releases and the release; `audit.yml`; `cache-cleanup.yml`;
`dependabot-merge.yml`). All logic is in `cargo xtask` and the scripts
`scripts/ci/*`, `.github/scripts/*`; packaging manifests are in `packaging/`;
the design and secrets are in `docs/ci.md`; branches and releases are in `docs/workflow.md`.
PRs, runs and logs go through `gh` (`gh pr create --base development`,
`gh pr checks`, `gh run view <id> --log-failed`). After editing a workflow file
check it with `mise x actionlint@1.7.12 -- actionlint`. GitLab is only a mirror without CI.

- **Branches.** `feature/*` (from `development`) and `hotfix/*` (from `main`) — checks
  only; `development` — pre-release `X.Y.Z-N` on every push; `main` —
  release `X.Y.Z` on every merge. Work goes through PRs, CI creates tags.
- **A release waits for checks.** Builds run in parallel with the checks (they need
  only `version` and the images), publishing (`publish` and everything after it) requires
  green `checks`, a red check cancels the run (`cancel-on-red`).
- **Version and CHANGELOG — only via `mise run bump` / `mise run changelog`**
  (git-cliff by Conventional Commits): the version in files is not changed by hand, the commit
  type (`feat`/`fix`/`perf`/…) decides where it lands and what the next
  version will be. CI will not release from `main` without a `## [X.Y.Z]` section in
  `CHANGELOG.md`.
- **`*.ps1` — ASCII only** (Windows PowerShell 5.1 reads them as cp1252:
  "—" breaks parsing); `mise run check-ci` (`scripts/ci/lint-ci.sh`) catches it.

- **CI repeats the local gate.** A new check in `mise run check`
  is added to `cargo xtask check` as well and into a group of `crates/xtask/src/checks/groups.rs`
  (the `check-linux`/`frontend-static`/`frontend-e2e` jobs in `ci.yml` select by groups and
  exclusions, so a new task runs in one of them by default; a test fails for a task in no group); CI must not be softer than `mise run check`. Keep the
  logic in xtask or a script (`scripts/ci/*`, `.github/scripts/*`), not in a
  multi-line `run:` — then it can be run and checked locally.
- **Tool versions — only from `mise.toml`.** CI installs node/pnpm/rust
  via mise (jdx/mise-action on Windows too); do not hardcode versions in YAML.
- **CI images.** Linux jobs run in prebuilt images
  (`scripts/ci/ci.Dockerfile`, images `ci-node`/`ci-linux`/`ci-android` in ghcr).
  The tag is a hash of the inputs (the Dockerfile, `apt-packages.sh`, `install-chrome.sh`,
  `[tools]` from `mise.toml`; `mise run ci-image-tag`), it is not written by hand:
  `images.yml` builds the missing images under the new tag itself. A new apt package goes
  only into `apt-packages.sh` (both the image and `setup-linux.sh` read it).
- **Do not lose speed.** Do not remove `opt-level = 3` for `argon2`/`blake2` in
  `[profile.dev.package.*]` (without it tests take 13 times longer); 
  sccache is written only by the `deps` job in `ci.yml` (the others read it); a new job
  gets `concurrency` that cancels older runs if it can be dropped.
  Details: `docs/ci.md`, the "Speed" section.
- **Platform code.** Tray, autostart, global shortcuts, window geometry
  and the OS keyring exist only on desktop: code goes under `#[cfg(desktop)]` with a
  stub under `#[cfg(mobile)]`, plugins go in
  `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`
  of `crates/app/Cargo.toml`. Otherwise `build:android` breaks. Check without CI —
  `mise run check-platforms` (see "Building on all platforms").
- **Identifiers do not change.** Two channels — two apps side by side
  (`crates/xtask/src/channel.rs`, `docs/ci.md` "Channels"): the release from `main` —
  `mwt.desktop` (desktop; the vault path depends on it), `mwt.app` (Android),
  `mwt.app.MultiWalletTracker` (Flatpak); dev from `development` and local ones —
  `dev.mwt.desktop`, `dev.mwt.app` (`tauri.conf.json`,
  `tauri.android.conf.json`), `dev.mwt.MultiWalletTracker`. In
  Rust the identifier is only `app_id::get()`, not a constant. A change = users lose data
  or cannot update. The former name TradeApp (`dev.tradeapp.desktop`)
  is migrated once at startup — `crates/app/src/legacy.rs`; do not delete
  it while anyone may still have the old installation.
- **Name.** The app is Multi Wallet Tracker (MWT): `productName`, binary and packages
  `multi-wallet-tracker`, AUR `multi-wallet-tracker-bin`; the dev channel —
  "Multi Wallet Tracker Dev", `multi-wallet-tracker-dev`. The Rust crates `trade-core`/`trade-app`
  remain internal names. Do not touch the vault magic bytes and the export format
  `trade-app-workflow` — reading old files depends on them.
- **Release — tag `vX.Y.Z`.** The version is the same in `crates/app/tauri.conf.json`,
  `[workspace.package]` of `Cargo.toml` and `<release>` in
  `packaging/flatpak/*.metainfo.xml`; `version:check` checks the tag. A tag and
  `jj git push` — only when the user asks.
- **Secrets — only in GitHub secrets** (Android signing key, updater key
  `TAURI_SIGNING_*`, AUR SSH key). No keys or keystores in the repository.
- **Generated files are not committed:** the Gradle project `crates/app/gen/android/`,
  `release/`, `.cache/`, `.flatpak-*` (in `.gitignore`).

## Checks and commits (Jujutsu)

The repository is under **jj** (colocated with git). The working copy is snapshotted
automatically into change `@` — `git add` does not exist and is not needed.

### Working copies: only alongside, in `../work-app-<task>`

The user must see where the work goes and be able to open it
in their editor. Therefore:

- A small task — right in the main copy: `jjt <slug>` → edits →
  gate → `jjc "<topic>"` (the `jj-tooling` skill from `../jj-tools`).
- **The agent runs the checks itself, not jj-tools.** The scripts `jjc`,
  `jjsplit`, `jjw-finish` do not run the gate: the agent runs `cargo fmt` +
  `mise run check` (and `check-platforms`) with its own commands, so that the
  user sees every run and its outcome.
- A big task (many files, experiments, several commits) or
  parallel work — **a separate jj workspace next to the repository**:
  `jjw-start <slug>` creates `../work-app-<slug>/` and the bookmark `<slug>`.
  The agent tells the user the path to this folder in the very first message
  about the task.
- **Forbidden** to create working copies inside the repository or in hidden
  directories: `git worktree add`, `.claude/worktrees/*`, `EnterWorktree` and
  `isolation: worktree` in Claude Code. The user cannot
  open such a copy or switch to it, it is not visible what changes in it, and its
  `biome.json` breaks `mise run check` in the main copy.
- When done — the gate and `mise run check-platforms` in that folder, then
  `jjw-finish <slug>` **from the main copy** (run from inside the working
  copy it transfers nothing): it moves the changes into the main copy and
  deletes the folder. After it — the gate again in the main copy, on the merged
  tree. A copy does not outlive its task; `jjws` checks that no leftovers
  remain.
- Commits inside `../work-app-*` — plain `jj describe` + `jj new`
  (`jjc` refuses to work there), the gate — `mise run check` in that folder.

### Gate before committing

```bash
cargo fmt             # formatting first, then the checks
mise run check        # cargo xtask: all checks + clippy + tests + gen-types + platforms, fail-fast
```

Gate green → commit. Not green → commit forbidden, fix first.
Never "fix" a failing test by deleting or weakening the check.

### Building on all platforms

After the **last** edit, before merging into the main copy (`jjw-finish`) and
before committing, check that the project compiles on all platforms:

```bash
mise run check-platforms   # linux, windows (cargo-xwin), android (NDK or the ci-android image), macos
```

- Code under `#[cfg(...)]`, platform dependencies and `platform_fs` do not
  compile on the host: `mise run check` does not see them, the error surfaces only in
  CI or at the user's (that is how an `unused import` under Android slipped through in
  `fix(sync)` on 2026-10-18).
- Any edit after the check — check again: only a run on the
  final state counts.
- `FAIL` — merging and committing are forbidden, as with a red gate.
- `SKIP` is acceptable only as a system limitation (macOS on Linux without
  `MACOS_BUILD_HOST`, no cargo-xwin / NDK / podman). Do not "fix" it by
  skipping: name the skipped platform and the reason to the user.
- Full bundles — `mise run build:all`; it is not needed to check compilation.

Faster, when the edit is clearly local:

| Changes | Command |
|---|---|
| `src/**` (frontend) | `mise run lint && mise run typecheck && mise run test-frontend` |
| `crates/core/**`, `crates/app/**` | `mise run test && mise run clippy` |
| DTOs, models, `ipc_contract.rs` | `mise run test && mise run gen-types` — be sure to diff `src/shared/api/ipc.gen.d.ts`, it must change |
| final, before committing | `mise run check` |

### Commit

```bash
jj status        # what lands in change @ (including uncommitted — jj has already counted it)
jj diff          # what exactly changed
jj describe -m "fix: ..."   # describe the current change
jj new           # next change on top of the current one
```

`jj commit` = `describe` + `new`. One change — one change; do not mix a
refactoring with a feature in one message.

**Message:** conventional commits, **in English**, subject ≤ 72 characters.
Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`. Body — if the
change is not obvious: *what* changed and *why*, not *which files were touched*
(that is visible in the diff).

### Co-author: the agent names itself

Every commit made by an agent carries a trailer with the **name of the tool**
the agent works with:

```
Co-authored-by: <Tool> <email>
```

- jj does **not** add it: `templates.commit_trailers` in the repo config
  is turned off deliberately, otherwise every agent would sign with someone else's name.
- The agent appends the line **itself**, as the last paragraph of the message, exactly one
  per tool; it does not remove other people's trailers when rewriting a message.
- The table of names and the order of checking are in the `commit-coauthor` skill.

**Forbidden:** `git commit`, `git add`, `git stash` (jj takes snapshots itself),
rewriting a change that has already gone to the server (`jj git push`).

A commit — **only on the user's explicit request**.

Nuances (split, squash, bookmark instead of branches, rebase, undo, work with
uncommitted state) are in the `jj-workflow` skill
(`.agents/skills/jj-workflow/SKILL.md`). Committing a non-empty working
state leaves junk in history — look at `jj diff` first.

## Agent skills

**All skills live in `.agents/skills/<name>/SKILL.md`** — this is the only
place. The `.opencode/skills/` directory is gone. Load the one you need **before** editing
code, not after:

| Skill | When |
|---|---|
| `workflow` | start of every session: the order of work, delegation to subagents, working copies |
| `jj-workflow` | commits, checks, branches, undoing changes |
| `commit-coauthor` | the `Co-authored-by` trailer with the name of your tool in every commit |
| `feature-sliced-design` | placing code by layers and import boundaries (methodology; project differences are in `frontend-feature`) |
| `ipc-command` | a new or changed Tauri command (DTO → core → handler → gen-types) |
| `vault-crypto` | vault format, migrations, crypto, `Amount`, secrets |
| `frontend-feature` | frontend architecture: layers, Query/Pinia, IPC and mock, query keys, i18n, routes, charts |
| `ui-design-system` | UI look and behavior: catalog of shared components, tokens, forms, action notifications, loading/error/empty, tables and money, keyboard, texts |
| `shadcn-vue` | any shadcn component: add, find, fix, compose |
| `json-editor` | the vault editor in JSON format (Settings → View JSON) |
| `pricing-offline` | prices/FX, providers, cache, offline degradation |
| `onboarding` | any user-visible feature: tours, hints, empty states, "What's new", the learning center (rule 12) |
| `subtask-pipeline` | any plan task that can be handed to a subagent: brief, Sonnet executor, Opus review (see "Subtasks") |
| `planning-and-task-breakdown` | breaking a stage into verifiable subtasks |
| `incremental-implementation` | an edit of more than one file: in thin slices, a check after each |
| `context-engineering` | subtask brief, working context of a new session |
| `debugging-and-error-recovery` | it crashed, hung, behaves wrongly: the cause before the fix |
| `code-review-and-quality` | reviewing a subtask result and your own diff before committing |
| `doubt-driven-development` | a risky decision (vault, crypto, money): review with a fresh eye |
| `code-simplification` | the code works but is more complex than needed; a file hit the line limit |
| `test-driven-development` | logic in `crates/core`: a failing test first (there are no frontend tests) |
| `source-driven-development` | a library API (vue-flow, reka-ui, TanStack, Tauri) — from the docs, not from memory |
| `security-and-hardening` | untrusted input: file import, attachments, links, sync |
| `performance-optimization` | noticeable slowdowns, a `check-layout` regression, heavy lists and charts |
| `graphify` | questions about code structure and file relations, if `graphify-out/` is built |

### Managing skills

Skills are installed and updated with the `skills` utility (the npm package `skills`,
run through the project's package runner). There is one directory — `.agents/skills/`,
OpenCode, Claude Code, Codex and Zed read it.

```bash
pnpm dlx skills add <registry> --agent '*' -y   # install/update a skill
pnpm dlx skills list                             # what is installed and who reads it
```

- `--agent '*' -y` are required: without them the CLI wants a TTY and dies in an agent
  environment ("Interactive prompt required but stdin is not a TTY").
- Update a registry skill (`Source: <repo>`) only with this command, do not
  edit by hand — the update will land on top.
- A skill with `Source: local` is written by hand in `.agents/skills/` and is updated
  together with the project code.
- Every `SKILL.md` must have YAML frontmatter with `name` and `description`,
  otherwise the skill silently drops out of loading (shown in `skills list` as `Skipped`).
- `shadcn-vue` is installed from `unovue/shadcn-vue`; it reads `components.json`,
  which lies in the root and describes the project aliases.
- Skills are installed **only** into `.agents/`.
- `.claude/skills/` — links to `.agents/skills/<name>`, one per skill,
  so that Claude Code finds skills itself (the Skill tool), not only by the
  table above. `scripts/link-claude-skills.sh` builds them: after
  `skills add` delete the `agent/` copy the CLI creates and rerun the
  script. Links go to all skills without exception; the skill files themselves are not
  put into `.claude/`.
- The plan and the task list are **not in the repository**. Registry skills
  (`planning-and-task-breakdown`, `spec-driven-development`) write
  `tasks/plan.md` and `tasks/todo.md` by default — here that is forbidden: the plan is a file
  in `~/.claude/plans/` (in OpenCode and Codex — their plans directory outside the
  repository), subtasks are the session task list. Do not create a `tasks/` directory in the working
  copy; if it appeared — delete it before committing.
- `addyosmani/agent-skills` is installed as `skills add addyosmani/agent-skills`
  as a whole. The repository's shared checklists (`references/`) are not copied by the CLI
  (issue #361); the needed ones live in `.agents/references/`, which the links
  `../../references/…` from the skills point to. When updating skills, check them against
  the repository.
- `graphify` — a Python CLI (`uv tool install graphifyy`, the package with two `y`),
  the skill is installed with `graphify install --project --platform agents`: it writes only
  `.agents/skills/graphify/`. A code graph without an LLM — `graphify update .`,
  the result in `graphify-out/` (in `.gitignore`).

## Subtasks

A strong model (Opus) writes the plan, breaks a stage into subtasks, writes the brief and
verifies the result; a subtask is executed by a subagent on a weaker model
(Sonnet). The executor does not commit and does not widen the task; the gate, commit and
report belong only to the planner, after their own check of the diff, checks and snapshots.
One executor at a time in a working copy if subtasks share files or
Vite. Brief, launch, review, return for rework — the `subtask-pipeline` skill.

The plan and the task list are **not in the repository**: `planning-and-task-breakdown` and
`spec-driven-development` write `tasks/plan.md` and `tasks/todo.md` by default,
here that is forbidden. The stage plan is a file in `~/.claude/plans/` (in OpenCode and
Codex — their plans directory outside the repository), subtasks are the session task list
(TaskCreate/TaskUpdate), its number `#<N>` goes into the brief. Do not create a `tasks/` directory
in the working copy; if it appeared — delete it before committing.

## Definition of Done

- [ ] `cargo fmt` done, no diff from autoformatting
- [ ] `mise run check` — green (all checks, clippy, `cargo test --workspace`, gen-types, platforms)
- [ ] `mise run check-platforms` on the final state — without `FAIL`; every
      `SKIP` named to the user with the reason
- [ ] `mise run gen-types` — `src/shared/api/ipc.gen.d.ts` is up to date (if DTOs changed)
- [ ] New DTOs in `ipc_contract.rs` are covered by ts-rs and a command in `crates/app`
- [ ] The command is registered in `generate_handler!` and in the `docs/ipc.md` table
- [ ] UI texts are added to both locales (`src/shared/config/i18n/locales/{ru,en}.json`)
- [ ] Onboarding for the feature (rule 12, the `onboarding` skill): tour, hints, empty
      states, "What's new" updated and `mise run check-onboarding` green —
      or the line `Onboarding: n/a — <reason>` in the commit
- [ ] Forms (rule 13): required fields with `*`, the save button is active,
      errors under fields via `useFormValidation`; `mise run check-forms` green
- [ ] Choosing an account, asset, currency (rule 14) — `StoreNodeSelect` /
      `AccountRefPicker` / `AssetInput` / `AssetSelect` / `CurrencyPicker`
- [ ] Shared components and the outcome of every action (rule 15):
      `mise run check-ui` green, the debt baseline did not grow
- [ ] Stability (rule 17): `mise run check-layout` green (full, not
      only `--quick` from `check`), if the interface changed; in the Tauri window on
      Linux (WebKitGTK) hover on rows, filter change,
      dialog opening, quick saves were checked — nothing jumps or freezes
- [ ] Phone (rule 16): the screen is checked at 390 and 360 px with inset
      emulation, without horizontal page scroll, every action
      is reachable by touch
- [ ] `jj diff` contains only what belongs to the task
- [ ] A commit is made **only if the user asked**, the gate before it is green
