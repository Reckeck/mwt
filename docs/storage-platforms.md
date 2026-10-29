# Vault files on every platform

How the app finds, opens, writes and shows the vault file on Linux,
Windows, macOS, Android and iOS. The module has two halves:

| Half | Where | What it does |
|---|---|---|
| core | `crates/core/src/vault_io/` | no Tauri, unit-tested: local file (atomic write, lock, `.bak`), path or URI, importing a copy, mirror logic |
| app | `crates/app/src/platform_fs/` | the only place in app with file dialogs, `content://` and the vault directory; picks the implementation by OS |
| Android | `crates/plugin-saf/` | Kotlin plugin: picking a document with a persistent access permission |

## The main rule

**The working vault is always a local file.** Core writes it through
`vault_io::local::atomic_write` (tmp, fsync, rename in the same folder), keeps
a `<vault>.lock` next to it and places `.bak` and `.vN.bak`. All of this needs a folder next to
the file. A document chosen through the Android Storage Access Framework
has none: it is a `content://…` URI, which `std::fs` cannot open.

So platform differences are reduced to three questions:

1. **Where the file came from.** On desktop the chosen path itself is used. On
   mobile the chosen document is copied into app storage (`import`).
2. **Where files go by default.** That is `platform_fs::vaults_dir` = `app_data_dir()`.
3. **A copy in an external file** (optional). After every save the
   sealed vault bytes are written into the chosen document (`mirror`).

## Platforms

| | Linux | Windows | macOS | Android | iOS |
|---|---|---|---|---|---|
| `VaultStatus.storage` | `files` | `files` | `files` | `app` | `app` |
| Default directory | `~/.local/share/dev.mwt.desktop/` | `%APPDATA%\dev.mwt.desktop\` | `~/Library/Application Support/dev.mwt.desktop/` | app storage `dev.mwt.app` | app storage |
| "Open file" | the path from the dialog, the file is opened in place | same | same | `ACTION_GET_CONTENT` → `content://` → a copy into the default directory | `file://` (security-scoped) → a copy |
| "Create" | the path from the save dialog or typed in; a bare name → the default directory | same | same | name only → `<directory>/<name>.wallet` | same |
| Recent | `recent_vaults.json` | same | same | `recent_vaults.json` + all `*.wallet` in the directory | same |
| Copy in an external file | any path (dialog) | same | same | `ACTION_OPEN_DOCUMENT`/`CREATE_DOCUMENT` + `takePersistableUriPermission` (the `saf` plugin) | dialog; the permission may not survive a restart |
| Lock of a live process | `/proc/<pid>` | lock is not checked (taken over) | same | `/proc/<pid>` | lock is not checked |
| CSV/JSON export | path | path | path | `ACTION_CREATE_DOCUMENT` → write to the URI | URL from the dialog |

The table describes the dev app; a release from `main` has `mwt.desktop` /
`mwt.app` instead of `dev.mwt.desktop` / `dev.mwt.app` (`docs/ci.md`, "Channels").

## Data flow

```
UI ─ pick_vault_file ─► platform_fs::pick_vault
                         ├ desktop: path (+ .wallet on save)
                         └ mobile:  URI → read_location → import_into(vaults_dir) → copy path
   ─ unlock_vault(path) ─► resolve_vault_to_open   (a URI from old recents is imported too)
                         ─► links::sync_before_open (reconcile with the copy: push / pull / merge)
                         ─► UnlockedVault::open     (core: read, lock, migration, .vN.bak)
                         ─► links::take_in          (pull/merge of the copy into the open vault, the password comes from the request, is not stored)
                         ─► attach_mirror           (if linked: the copy thread)
   ─ create_vault(path) ─► resolve_new_vault        (bare name → vaults_dir, URI forbidden)
mutation → UnlockedVault::persist → dirty + wakes vault_writer (batch)
vault_writer: 500 ms of quiet (at most 3 s) → prepare_flush (under the lock: sync marks, encode)
            → FlushJob::write (without the lock: seal with the session key → atomic_write)
            └► VaultMirror::push(bytes) → thread → write_location(URI, "wt")
```

### Batched writes

Changes accumulate in memory and are written with a single save per batch
(`crates/core/src/service/saving.rs`, `crates/app/src/vault_writer.rs`).
Reads come from memory, so the interface sees a change immediately.

Forced (`persist_now` / `flush_pending`, the file is on disk before returning):
vault creation, password change, format change, a copy before replacing the document and
before sync, vault transfer, linking an external copy, locking
(on error the vault stays open), switching to another vault, exit
(`RunEvent::ExitRequested` — on error the exit is cancelled and the window offers
"Retry" / "Quit without saving"), hiding the window to the tray and the page going to the
background (`visibilitychange` → `flush_vault`: Android may kill a background process).
The `Drop` of an open vault writes anything unsaved as a last resort.

The order of writes is never rolled back: each job carries a revision, a shared "gate"
remembers the last one written, and a late older job is skipped.
A write error leaves the batch unsaved; the writer retries it after 1, 2,
5, 10 s and reports `vault-save-failed` (later `vault-saved`).

**Loss window:** on a normal exit, lock and going to the background — zero; on
a process crash or power loss — at most ~3 s of the latest edits.

## A copy in an external file

- **Link storage.** `<app config>/vault_links.json`:
  `{ "<vault path>": { target, last_synced_hash, last_synced_at, last_error } }`.
  There are no secrets there: the copy itself is the same encrypted file.
- **Writing.** `mirror_sink`: a dedicated thread for every linked vault.
  `persist` holds the vault mutex, while a cloud provider may write for seconds,
  so the write happens in the background. Accumulated saves collapse into the
  last one. A write error is stored in `last_error` and arrives as the
  `vault-mirror-failed` event; the local vault does not suffer.
  The write goes through compare-and-swap (`vault_io::mirror::compare_and_push`):
  before writing, the copy is read, and if its hash is not equal to `last_synced_hash`
  (and not equal to the bytes being written), the copy is **not overwritten**. The link
  gets `remote_changed = true` and a `last_error` of the form `VAULT_REMOTE_CHANGED: <text>` (a code for translation in the UI; it arrives
  as the `vault-mirror-failed` event and is visible in the link status), and writing is paused
  until the next sign-in. A new link (`link_vault_external`) replaces the chosen
  file without comparing: the user chose so.
- **Reading at sign-in.** `vault_io::mirror::reconcile(local, remote, last_synced_hash)`:

| What changed since the last sync | Action |
|---|---|
| nothing, bytes are equal | `InSync` |
| only the local one (or there is no copy) | `PushLocal` — write the copy |
| only the copy (an edit on another device) | `PullRemote` — the local one goes to `.bak`, the copy's data becomes the vault's data, `stamps` and `tombstones` are taken from the copy (they are record metadata), `peers` are merged by the `merge_peers` rules, and the undo batches, the journal, `transfers`, `tombstones_seen`, `paused`, `cut_by` **stay local** |
| both (including a copy written blindly by an old app) | `Conflict` → a **merge** into the open vault, with no questions |

  The merge (`links::take_in`, `UnlockedVault::merge_external_copy`): the copy
  is opened with the same password (it exists only during `unlock_vault`); if
  `doc.id` matches, its records are applied through the peer sync path
  (`sync_manifest`/`sync_records` on the copy, `sync_apply` on the local vault,
  the "External copy" partner). The batch is visible in the sync history and
  is undone like any other. The copy's `doc.sync` is not imported. Before the
  merge the file stays in `<vault>.bak`, and after saving the result
  is written to the copy (again with the check). If the copy does not open with this password or it is another
  vault, the old path remains: the `SYNC_CONFLICT` error, the UI offers
  "Keep this vault" / "Take the copy from the file" (`resolve_vault_conflict`).

  An unreachable copy (permission revoked, disk offline) does not prevent sign-in: the error
  is written to `last_error`, the local file is opened.
- **Linking.** `link_vault_external { existing }`. With `existing = true`
  the chosen file is **replaced** by the current vault (the UI says so in a
  hint); with `false` a new one is created. Someone else's copy can be pulled in by
  opening the file (import) or through `PullRemote` at sign-in.

## Relations to other modules

- `storage` and `crypto` re-export `vault_io::local` (`atomic_write`,
  `read_all`, `VaultLock`, `backup_path`, `ensure_wallet_ext`, `VAULT_EXT`):
  the old import paths work.
- `service::UnlockedVault` only knows about `VaultMirror` (a trait) and
  `set_mirror`; the sandbox gets no mirror.
- `commands/auth.rs` and `commands/vault_recents.rs` call `platform_fs`
  instead of `PathBuf::from` and dialogs. `commands/vault_link.rs` is a thin
  wrapper over `platform_fs::links`.
- `analytics::export_report_csv` and `settings::dev_export_vault` write through
  `platform_fs::save_bytes_dialog`.
- `app_dirs`/`legacy.rs` (the `dev.tradeapp.*` migration) are unchanged: it is a
  one-time directory migration on desktop.
- `appconfig`, `notify_store`, `recent_vaults.json`, `vault_links.json` live
  in `app_config_dir()`: these are plain app files on every OS.

## Verification

- `cargo test -p trade-core vault_io::`: paths of all OSes, `file://`,
  `content://`, file name, import without overwriting, all `reconcile` branches.
- `cargo test -p trade-core a_mirror_gets_exactly_the_saved_file`.
- `cargo test -p trade-core cas_` (compare-and-swap), `cargo test -p trade-core merge_` and
  `a_pull_from` (merge and pull keeping the local `doc.sync`).
- `cargo test -p trade-app platform_fs`.
- `cargo clippy -p trade-app --lib --target aarch64-linux-android -- -D warnings`
  (needs the NDK) or `ANDROID_TARGETS=aarch64 mise run build:all -- android`.
- `pnpm dev`, then `/unlock?storage=app`: the Android/iOS screens in the mock; the path
  with `conflict` answers `SYNC_CONFLICT`.
