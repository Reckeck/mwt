# Architecture

## Overview

Multi Wallet Tracker (MWT) is a Tauri 2 desktop application with a monolithic Rust core and a Vue
frontend, connected through IPC (invoke + events). All business logic lives in `crates/core`
(pure Rust, zero dependencies on Tauri) and is covered by unit tests; `crates/app`
is a thin shell: `tauri::Builder`, commands and state.

```
┌─────────────────────────────────────────────────────┐
│                    Vue 3 frontend                    │
│  Pinia (state) · TanStack Query (network + cache)   │
│  shadcn-vue (UI) · vue-echarts (charts)             │
│  vue-i18n (ru/en) · types from src/shared/api/ipc.gen.d.ts      │
└──────────────────────────┬──────────────────────────┘
                           │ invoke / events
┌──────────────────────────┴──────────────────────────┐
│                crates/app (Tauri shell)             │
│  AppState { vault: Mutex<Option<UnlockedVault>>,    │
│             vault_path: Mutex<PathBuf>, pricing }   │
│  commands/{auth,stores,transactions,categories,     │
│             pricing,portfolio,settings,exchange,     │
│             app_settings,vault_recents}             │
└──────────────────────────┬──────────────────────────┘
                           │ (shared dependency via the workspace)
┌──────────────────────────┴──────────────────────────┐
│                crates/core (business)               │
│  model/      — domain entities (VaultDoc, Store,    │
│                Transaction, Settings, Chain)        │
│  appconfig.rs— app settings (outside the vault, JSON)│
│  service.rs  — UnlockedVault: all mutations + analytics│
│  pricing/    — price providers, cache, FX            │
│  crypto/     — XChaCha20-Poly1305 + Argon2id,        │
│                fixed 55-byte header (AAD)            │
│  storage/    — SnapshotFormat (JSON/MessagePack),    │
│                atomic write                          │
│  market/     — Binance OHLCV candles for charts      │
│  ipc_contract.rs — command DTOs (ts-rs → ipc.gen.d.ts)│
└─────────────────────────────────────────────────────┘
```

## Android plugins

Platform-specific Android code lives in separate plugin crates with a Kotlin part; they expose
nothing to the webview, Rust calls them (`run_mobile_plugin`):

- `crates/plugin-saf` — picking a document with a persistent permission (`platform_fs`);
- `crates/plugin-net` — local-network sync: multicast lock, foreground
  service, device key in the Keystore;
- `crates/plugin-insets` — system bars: status bar, navigation, cutout and keyboard
  insets as the CSS variables `--native-inset-*` and `--native-ime` (WebView reports
  `env(safe-area-inset-*)` as zeros), icon color matching the theme
  (`set_system_bars`) and "move to back" (`move_to_back`). The frontend reads them
  through `--safe-*` and the `*-safe` utilities (`src/app/styles.css`).

## Where things live on disk

Paths are for the dev app (`dev.mwt.desktop`); a release from `main` stores everything
the same way, but in `mwt.desktop` directories (channels — `docs/ci.md`).

| Path (Linux) | What | Format |
|---|---|---|
| `~/.local/share/dev.mwt.desktop/vault.wallet` | the vault: vault settings, stores, transactions, **price and FX cache** | encrypted (JSON in dev, MessagePack in release; the old bincode is still readable) |
| `~/.config/dev.mwt.desktop/app_settings.json` | theme, language, base currency, favorite currencies | plain JSON |
| `~/.config/dev.mwt.desktop/currencies.json` | currency catalog cache (fiat + crypto) | plain JSON |
| `~/.config/dev.mwt.desktop/recent_vaults.json` | recent vaults | plain JSON |
| `~/.config/dev.mwt.desktop/vault_links.json` | which vault is mirrored to which external file | plain JSON |

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
2. All mutations (store/tx/settings) go through `UnlockedVault::*` methods,
   each of which ends with `persist()` — re-encryption and an
   atomic write to disk.
3. Prices: `refresh_all` first takes a snapshot of what it needs (assets, base currency)
   and releases the mutex, then does the network fetches (library await),
   then locks the vault again and applies the cache through `apply_prices/apply_fx`.
4. The frontend listens for the `prices-updated` event and invalidates the TanStack cache.
5. User settings do not go through the vault: `App.vue` reads
   `get_app_settings` before unlock, so the theme and language are applied already on
   the sign-in screen; after `open`/`create` the base currency is pushed into the vault
   through `sync_base_currency`.

## Principles

- **A core without a GUI.** Everything testable is in core, tests run without Tauri.
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
  field names (before v2 — bincode, now read-only). The type is stored in the file
  header (`payload_fmt`), so the file is self-contained. Old schema versions are migrated
  on open, see `docs/data-model.md` → "Versions and migrations".

## Scaling

For now it is a monolith. If the core grows, potential slots are: moving heavy
numeric tasks into a separate background process, data-provider plugins through
feature flags in `trade-core`. The IPC contract remains the source of truth for the frontend.
