# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

The main source of rules is `AGENTS.md` (shared by all agents); it is imported
in full below and takes priority. This file holds only additions that are not there.

@AGENTS.md

## Running individual tests

Rust tests are inline `#[cfg(test)]` modules inside `crates/core/src/**` (there is no
separate `tests/` folder). Frontend tests are vitest, project `unit`: `*.test.ts` next to the code, `mise run test-frontend`
(`pnpm exec vitest run --project unit <part of the file name>` for a single one). `call()`/`onEvent` and
i18n (the real ru locale) are mocked in `src/test/setup.ts`. End-to-end scenarios are the `e2e` project
(`tests/e2e/flows/*.e2e.ts`, `mise run test-e2e`, the `app` fixture from `tests/e2e/harness/app.ts`: `unlock()`,
`at(route, {width, theme})`, `waitFor`, `screenshot`; a failed test leaves a screenshot in `.cache/e2e/`).
Static checks (onboarding, routes, forms, ui, fsd) are the `checks` project, `tests/checks/*.check.ts`
(a rule = `describe`/`it`, a violation = a failed test with a line `error: file:line: …`); the ui rules live in
`tests/checks/rules/ui/`. Run with `mise run check-<name>` (`-- --update` for ui and fsd rewrites the baseline
`scripts/check-*.baseline.json`) or the whole set with `pnpm exec vitest run --project checks`.
`mise run check-layout` and `check-links` are the `layout` project (`tests/e2e/layout/`, `tests/e2e/links/`, probes in
`lib/`), with flags `--quick`, `--report`, `--update`, `--overlays=id`, `--jobs N` (processes of one machine),
`--shard K/N` (a share of the units, the CI matrix), `--update-weights` (writes `weights.json` from the measurements in
`.cache/check-<name>/timings.json`) via `scripts/run-browser-check.mjs`; the split lives in `tests/e2e/harness/shard.ts`;
run them as separate processes (a touch in layout is irreversible for the pointer in the same browser).

```bash
cargo test -p trade-core                      # core only
cargo test -p trade-core service::            # by module path prefix
cargo test -p trade-core <name> -- --exact --nocapture
```

`CARGO_TARGET_DIR=.rust-target` is set in `mise.toml`; if cargo runs outside
mise, the artifacts end up in `target/` and the build starts from scratch.

## The big picture (what takes reading several files)

- **Vault lifecycle.** `auth::unlock_vault` → `VaultFile::open` (the AEAD check
  of the password = decryption, there is no verifier) → `UnlockedVault` is put into
  `AppState.vault: Mutex<Option<UnlockedVault>>`. Every mutation in
  `UnlockedVault` (`service.rs`) ends with `persist()`: a full
  re-encryption and an atomic write (tmp + fsync + rename). There is no separate "save"
  from the frontend.
- **Vault files on different OSes.** The working vault is always a local path:
  `PathBuf::from` is never applied to a string from the frontend, only
  `platform_fs::resolve_vault_to_open/resolve_new_vault` (Android hands out
  `content://`, which has to be imported). File dialogs, URIs and the vault directory live
  only in `crates/app/src/platform_fs/`, the local write is in
  `trade_core::vault_io`. A linked vault hands its bytes to `VaultMirror` after `persist()`.
  Details: `docs/storage-platforms.md`.
- **Two settings stores.** Theme/language/base currency/favorites are in
  `appconfig.rs` (plain JSON in the config directory, available before unlock,
  read in `App.vue` via `get_app_settings`). Everything domain-related, including the price
  and FX cache, is inside the encrypted vault. After open/create the base currency
  is pushed into the vault via `sync_base_currency`.
- **Snapshot format.** `SnapshotFormat::for_profile()`: JSON in debug,
  MessagePack (`Pack`) in release; this is the *write* format; on read the format
  is taken from the `payload_fmt` byte in the 55-byte header. The old bincode
  (`Bin`) is read-only. `UnlockedVault::open` migrates an old schema version
  (`storage::decode_versioned`), puts `<vault>.vN.bak` next to it and immediately
  rewrites the file.
- **Prices.** A command takes the data it needs under the mutex, releases it, runs
  `PricingService::fetch_*` (network), takes the mutex again and calls
  `apply_*`; then it emits the `prices-updated` event, on which the frontend
  invalidates TanStack Query.
- **Errors over IPC.** Rust returns a string `"CODE: message"` (`Error::code()`),
  `parseError` in `src/shared/api/ipc.ts` parses it into `{ code, message }`.
- **`pnpm dev` without Tauri.** When `__TAURI_INTERNALS__` is absent, `call()` in `ipc.ts`
  lazily loads the in-memory mock `src/shared/api/mock/` (password: `secret`). A new
  command that has to run in the browser needs a branch in that mock.
- **Adding a command** touches: the DTO in `ipc_contract.rs` → logic in
  `service.rs`/core → the handler in `crates/app/src/commands/*` →
  `generate_handler![…]` in `crates/app/src/lib.rs` → `mise run gen-types` →
  `docs/ipc.md`. Details: the `ipc-command` skill.

## Teaching together with the feature

Rule 12 of `AGENTS.md`: a user-visible feature ships with a tour,
hints and a "What's new" entry. Load the `onboarding` skill before touching the UI;
the gate checks this with the `checks` project (`tests/checks/onboarding.check.ts`, `mise run check-onboarding`).

## UI: shared components and the outcome of every action

Rule 15 of `AGENTS.md`. Load the `ui-design-system` skill before laying out markup
(component catalog, the `references/` handbook). Notifications are shown by
`MutationCache` in `src/app/providers/query` from the mutation's `meta`; `toast` is not
called in a component. `mise run check-ui` keeps a baseline of old debts
(`scripts/check-ui.baseline.json`): once you fix one, run `mise run check-ui -- --update`.

## Working copy in Claude Code

The "Working copies" rule from `AGENTS.md` overrides Claude Code's built-in
isolation: **do not call `EnterWorktree`** and do not start agents with
`isolation: "worktree"`; they create a git worktree in `.claude/worktrees/`,
which the user cannot switch to. If the environment requires isolating
edits (a background task), run `jjw-start <slug>`, work in
`/home/wd/apps/work-app-<slug>/` and immediately tell the user that path.

## Commits from Claude Code

The jj rules from `AGENTS.md` apply in full: `git commit`/`git add` are forbidden,
commit only when the user asks. jj does **not** add the co-author trailer; append
`Co-authored-by: Claude Code <noreply@anthropic.com>` yourself as the last paragraph
(or the attribution line set by the environment), see the `commit-coauthor` skill.

## Start of a session in Claude Code

The first call is the `Skill` `workflow`. Do analysis, search and edits with the
`Agent` tool (without `isolation: "worktree"`); the main session verifies the
subagent's result (diff, gate) instead of retelling its report.
