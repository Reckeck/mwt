# Architecture

## Overview

Multi Wallet Tracker (MWT) is a Tauri 2 desktop application with a Rust backend split into crates and a
Vue frontend, connected through IPC (invoke + events). All business logic lives in the Rust crates under
`crates/` (pure Rust, zero dependencies on Tauri) and is covered by unit tests; `crates/app` is a thin
shell: `tauri::Builder`, commands and state. There is no god object: the open vault is one `Session`, the
rules are functions of the domain crates, and every command is one scenario in `crates/usecases`.

## Crates

The workspace is `crates/*`: 23 members. The edges are a table in `crates/xtask/src/arch/crates.rs`;
`cargo xtask check arch` fails on an edge that is not in it.

| Crate | What it holds | May depend on |
|---|---|---|
| `mwt-macros` | the `#[dto]` attribute (serde + feature-gated ts-rs / schemars derives) | nothing |
| `mwt-base` | `Amount`, errors, limits, time and calendar, calculators, secrets, route names | macros |
| `mwt-crypto` | the container codec, key derivation, AEAD seal and open, KDF profiles, `SnapshotFormat` | base |
| `mwt-model` | `VaultDoc` and the types it holds, ids, record kinds, coin tables, the data of the device secrets (`DeviceSecrets`) | base, macros, crypto |
| `mwt-net` | price and FX sources, exchange clients, data sources, market candles, updates, the chain RPC client | base, model |
| `mwt-vault` | the file codec, frozen v1-v4 schemas, vault files, copies, backups, mirrors, JSON stores, the kernel of the open vault | base, crypto, model |
| `mwt-sync` | device-to-device merge, the Noise channel, the pairing code, transfer | base, crypto, model |
| `mwt-money` | pure money rules: the books (`Books`), ledger, valuation, cost basis, validation | base, model |
| `mwt-household` | attachments, contacts, debts, losses, loyalty, property links, relations, shared events, utilities, vehicles | base, model, money |
| `mwt-workflow` | the node catalog, graph, validation, executor, record, run, mark and variable rules | base, model |
| `mwt-planning` | budgets, envelopes, goals, allocation, agenda, automations, health, notifications, scenarios | base, model, money, household |
| `mwt-reports` | overview, dashboard, insights, health, comparison, where-went, notification snapshot | planning, household, money, model, base |
| `mwt-cryptotrack` | the crypto module: chains, protocols, engine, sources, tokens, the pure service rules | base, model, money |
| `mwt-contract` | the command input/output types shared with the frontend (`#[dto]`) | the domain crates, sync, crypto, vault, net |
| `mwt-usecases` | `Session`, the scenarios (`usecases/`), the service shim, ports, sandbox, app config, the type generator `gen_types` (`src/bin`) | everything above |
| `mwt-testkit` | property strategies, an in-memory persistence, model-only builders (dev-only) | base, model, vault |
| `mwt-app-macros` | the `#[ipc]` attribute for `crates/app` commands | nothing |
| `trade-app` | the Tauri shell | usecases, app-macros, the plugins |
| `tauri-plugin-saf`, `-net`, `-biometric`, `-insets` | Android plugins (below) | Tauri only |
| `xtask` | the build and check engine (`cargo xtask`) | nothing |

### Session, scenarios, ports

- **`Session`** (`crates/usecases/src/service/mod.rs`) is the open vault: the document, the password-derived
  key, the file lock, the device-secrets state and the write state `kernel::session::Write`. It is the only
  owner of the open vault; `AppState.vault` holds it in a mutex.
- **A scenario** is a function in `crates/usecases/src/usecases/<area>/` (`money`, `household`, `planning`,
  `reports`, `cryptotrack`, `sync`, `workflow`) that takes `Books` (a read context) or a `Session`, calls
  the pure rules of the domain crates and returns a result. A command in `crates/app` calls one scenario.
- **The rule: a mutation is a scenario: decide, then one `transact`.** The scenario reads, decides and
  builds the change, then applies it in a single `Session::transact` (or `transact_now`). The domain
  crates never write the vault.
- **Ports** are traits owned by the crate that needs them and implemented where both sides are visible:
  `Persistence` and `ChangeSink` (kernel, `mwt-vault`), `Clock` (`mwt-base::time`), `ChainSource` and
  `PriceSource` (`mwt-cryptotrack`; the adapters are in `crates/usecases/src/service/ports.rs`),
  `BooksSource` (`mwt-money`; the `Session` implements it in `service/books.rs`, so the money rules stay
  below the vault).

```
┌─────────────────────────────────────────────────────┐
│                   Vue 3 frontend                    │
│  Pinia (state) · TanStack Query (network + cache)   │
│  shadcn-vue (UI) · vue-echarts (charts)             │
│  vue-i18n (ru/en) · types: shared/api/ipc.gen.d.ts  │
└──────────────────────────┬──────────────────────────┘
                           │ invoke / events
┌──────────────────────────┴──────────────────────────┐
│             crates/app (Tauri shell)                │
│  AppState { vault: Mutex<Option<Session>>,          │
│             vault_path: Mutex<PathBuf>, pricing }   │
│  commands/*: thin handlers, one scenario each;      │
│  scheduler, write-behind writer, platform_fs        │
└──────────────────────────┬──────────────────────────┘
                           │ scenarios over Session
┌──────────────────────────┴──────────────────────────┐
│         crates/usecases + the domain crates         │
│  mwt-usecases — Session, scenarios, ports, sandbox  │
│  mwt-contract — command DTOs (ts-rs -> ipc.gen.d.ts)│
│  mwt-vault    — file codec, kernel of the open vault│
│  mwt-model    — VaultDoc and the types it holds     │
│  domain crates, crypto, net, sync: see "Crates"     │
└─────────────────────────────────────────────────────┘
```

## Android plugins

Platform-specific Android code lives in separate plugin crates with a Kotlin part; they expose
nothing to the webview, Rust calls them (`run_mobile_plugin`):

- `crates/plugin-saf` — picking a document with a persistent permission (`platform_fs`);
- `crates/plugin-net` — local-network sync: multicast lock, foreground
  service, device key in the Keystore;
- `crates/plugin-biometric` — quick unlock: a Keystore key released only by `BiometricPrompt`
  with a `CryptoObject` (`biometric/android.rs`);
- `crates/plugin-insets` — system bars: status bar, navigation, cutout and keyboard
  insets as the CSS variables `--native-inset-*` and `--native-ime` (WebView reports
  `env(safe-area-inset-*)` as zeros); the window is `adjustResize` edge to edge, so the
  keyboard is lifted only by the page (`--safe-bottom`), never panned as well; icon color matching the theme
  (`set_system_bars`) and "move to back" (`move_to_back`). The frontend reads them
  through `--safe-*` and the `*-safe` utilities (`src/app/styles.css`).

## The crypto module

A user follows public addresses and exchanges; the module reads them and nothing is typed in by hand. The stored data
is nine sibling `crypto_*` fields of the vault (no nested object; `docs/data-model.md` -> "The crypto module").

- **The domain crate** (`crates/cryptotrack/src/`, `mwt-cryptotrack`) is pure and Tauri-free: `source.rs` (the traits), `sources/evm` (Blockscout
  history + node balances) and `sources/bybit.rs`, `rpc/` (a JSON-RPC client with a per-host limiter and a rule that
  one address's calls are never merged with another's), `tokens/` (identity by network + contract, quarantine),
  `prices.rs` (historic quotes, fetched by `crates/net/src/pricing/llama.rs`) and `engine/` (operations, the user's overrides, cost
  basis, reconciliation with the node, snapshots). Nothing derived is stored.
- **The vault side** is `crates/usecases/src/service/cryptotrack/` (and the pure rules in `crates/cryptotrack/src/service/`): `apply.rs` stores a batch, `recompute.rs` folds everything again and is
  the only place that knows "now", plus accounts, tokens, links, relations, `transition.rs` and `wipe.rs`.
- **The shell** (`crates/app`): `commands/cryptotrack/*` are thin IPC handlers; `cryptotrack/jobs.rs` and the scheduler
  run the background reads. A network call follows the usual pattern: copy the inputs under the mutex, release it,
  fetch, take it again and apply.
- **Device secrets.** The exchange keys, the per-network endpoints and the Blockscout key live in `<vault>.secrets`,
  encrypted with a key derived from the vault key, outside the vault bytes, so sync, the mirror and backups never carry
  them. The wipe removes this file.
- **Sync levels.** Crypto collections travel only at protection level 3; below it the sender omits them and the
  receiver refuses them. Cursors are device-local. A wipe is a synced record that other devices honour.
- **Entering and leaving.** Raising to level 3 archives the hand-kept crypto wallets (nothing deleted); the wipe makes
  a checked backup first, then clears the module and archives its wallets.
- **Frontend.** The module has one route of its own, `/crypto/positions` (below); everything else appears in wallets of the Crypto kind (assets, quarantine,
  operations, relations tabs), in the "Crypto operations" list under Transactions, and in Settings -> Crypto (networks,
  consent, hidden tokens, wiping). The browser mock lives in `src/shared/api/mock/commands-crypto-*.ts`.
- **DeFi positions** (`/crypto/positions`, `src/pages/crypto-positions`, slice `src/entities/crypto-position`) is a read-only
  screen over what the adapters found: the engine computes every figure and the page only composes them. A missing figure
  is a dash, never a zero, and `≈` marks what an adapter called approximate. Its onboarding is a route tour (list, net
  tile, fees), an empty state that leads to connecting an address on the wallets screen, hints on the tile labels and the
  breakdown note, and a "What's new" entry; texts are in `locales/parts/defi-onboarding.*`. The anchors inside the detail
  dialog have their own names and are listed in `CRYPTO_TOURLESS_TARGETS`.

## Where things live on disk

Paths are for the dev app (`dev.mwt.desktop`); a release from `main` stores everything
the same way, but in `mwt.desktop` directories (channels — `docs/ci.md`).

| Path (Linux) | What | Format |
|---|---|---|
| `~/.local/share/dev.mwt.desktop/vault.wallet` | the vault: vault settings, stores, transactions, **price and FX cache** | encrypted (JSON in dev, MessagePack in release) |
| `~/.config/dev.mwt.desktop/app_settings.json` | theme, language, base currency, favorite currencies | plain JSON |
| `~/.config/dev.mwt.desktop/currencies.json` | currency catalog cache (fiat + crypto) | plain JSON |
| `~/.config/dev.mwt.desktop/recent_vaults.json` | recent vaults | plain JSON |
| `~/.config/dev.mwt.desktop/vault_links.json` | which vault is mirrored to which external file | plain JSON |
| `<vault>.secrets` (next to the vault) | device secrets: exchange keys, per-network endpoints, Blockscout key; never synced or backed up | encrypted with a key derived from the vault key |

Paths on other OSes, Android (the vault in app storage, `content://` import) and
the copy in an external file — `docs/storage-platforms.md`.

The price and FX cache lives **inside the vault** (`VaultDoc.price_cache` / `fx_cache`) — it is
encrypted and survives a restart, but is tied to a particular vault, each one has its
own. The currency catalog is in the app configs: it is not secret and is the same for all
vaults.

## Data flow

1. The user enters the master password → `auth::unlock_vault` →
   `VaultFile::open(path, password)` → `crypto::open_bytes` (AEAD check of the
   password) → `VaultDoc` deserialization. The result is stored in `AppState.vault`.
2. Every mutation is a scenario: decide, then one `Session::transact`, which marks
   the document through `persist()`. The app's write-behind writer then does the
   re-encryption and the atomic write to disk, batching a burst of changes into one write.
   `persist_now` writes at once: it is used for copy, re-key and close. Without a
   hook (tests, tools) `persist()` writes at once.
3. Prices: `refresh_all` first takes a snapshot of what it needs (assets, base currency)
   and releases the mutex, then does the network fetches (library await),
   then locks the vault again and applies the cache through `apply_prices/apply_fx`.
4. The frontend listens for the `prices-updated` event and invalidates the TanStack cache.
5. User settings do not go through the vault: `App.vue` reads
   `get_app_settings` before unlock, so the theme and language are applied already on
   the sign-in screen; after `open`/`create` the base currency is pushed into the vault
   through `sync_base_currency`.

## Layers and ports

The crates of the table above form layers: a crate may use only the crates in its row. `crates/xtask/src/arch/crates.rs`
holds the crate table; `crates/xtask/src/arch/layers.rs` holds the module table of the part of `crates/usecases/src`
that has not moved yet. A new back edge fails `cargo xtask check arch`; to allow one, write an entry with `until` and a
reason in the PR (`crates/xtask/arch-allow.toml`, empty today). An entry that no longer matches an edge fails too.

| Layer | Crates |
|---|---|
| 0 | `mwt-macros`, `mwt-base` |
| 1 | `mwt-crypto` |
| 2 | `mwt-model` |
| 3 | `mwt-vault`, `mwt-sync`, `mwt-net` |
| 4 | `mwt-money`, `mwt-household`, `mwt-planning`, `mwt-reports`, `mwt-cryptotrack`, `mwt-workflow` |
| 5 | `mwt-contract`, `mwt-usecases` |
| 6 | `mwt-testkit` (dev-only) |

The kernel (`crates/vault/src/kernel/`, re-exported as `kernel::` by `mwt-usecases`) is the one place where a change of the open
vault is committed. What exists:

- `kernel::session::Write` — the write state of an open vault: dirty flag, revision, the write gate,
  the store and the event bus. The `Session` of `mwt-usecases` holds one; the secrets and the file lock stay
  in the `Session`.
- `Tx`, `Collection` — a transaction over the document and typed access to its records (the `Record` trait is in `mwt-model`; `Books`, the read context, is in `mwt-money`).
- `Persistence` (port) — where a sealed snapshot goes: `FileStore`, `SandboxStore`
  (no I/O), `Mirrored` (copy to an external document).
- `ChangeSink` (port) — told the keys of every local save, so a live sync session can send them on.
- `Clock` (port, `time.rs`) — `SystemClock` and `FixedClock` for tests.
- Event bus (`EventBus`, `DomainEvent`: `VaultLocked`, `Committed`, `KeysChanged`) —
  the write-behind hook and the sync tap are its subscribers.
- `SaveHook` and `Commit` — how a marked change is told to the writer.

The sandbox gate still lives in the service write path (`Session::transact` skips the write for a sandbox).

The facade's old name and the forwarding macro are banned for good: `xtask arch` refuses
them in every `crates/*/src/**/*.rs` file, comments and tests included.

## Principles

- **A backend without a GUI.** Everything testable is in the Rust crates, tests run without Tauri.
- **Secure by default.** One password, no stored verifiers;
  integrity is provided by AEAD (header = AAD, any change fails the check).
- **Offline-friendly.** Prices/FX accumulate history in the vault (`price_cache`,
  `fx_cache`); without a network the last known rate is used, and transactions
  are valued at the rate of their own date.
- **Money = Decimal.** `Amount` is a newtype over `rust_decimal::Decimal`,
  always serialized as a string (precision + stable JSON and MessagePack).
- **Settings that describe the user live outside the vault.** Theme, language,
  base currency and favorite currencies are not encrypted and not duplicated in each
  vault; they have nothing to hide, while a second vault should inherit them and
  the sign-in screen should render in the right language before the password is entered. Vault-scoped
  settings (intervals, tray, widgets) stay in the document.
- **Snapshot formats.** Dev — human-readable JSON, release — MessagePack with
  field names. The type is stored in the file header (`payload_fmt`), so the file
  is self-contained. The schema chain starts at v1; the only conversion from an
  older file is the frozen `legacy_v4` transition, see `docs/data-model.md` →
  "Versions and migrations".

## Scaling

The backend is already split into crates (above). If it grows, potential slots are: moving heavy
numeric tasks into a separate background process, data-provider plugins through
feature flags in `mwt-net`. The IPC contract remains the source of truth for the frontend.
