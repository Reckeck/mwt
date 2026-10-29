# Roadmap

Status legend: `[x]` done · `[~]` in progress · `[ ]` planned.

## Phase 1 — Foundation `[~]`
- `[x]` Cargo workspace (`crates/core` + `crates/app`), mise environment, tasks
- `[x]` Domain model (VaultDoc, Store/Holding, Transaction, Settings, Chain, Sim)
- `[x]` Crypto: Argon2id + XChaCha20-Poly1305, header=AAD, atomic write
- `[x]` Storage: JSON (dev) / bincode (release), VaultFile::create/open/save
- `[x]` UnlockedVault: mutations, holdings (derived), summary, history, realized PnL
- `[x]` Prices+FX: CoinGecko→CoinCap→Binance, open.er-api.com, TTL cache
- `[x]` IPC contract + the `src/shared/api/ipc.gen.d.ts` generator (ts-rs)
- `[x]` Tauri shell: builder, AppState, commands (auth/stores/…/sim)
- `[x]` Documentation (README, AGENTS, docs/*)
- `[~]` Frontend scaffold (see Phase 2)
- `[ ]` First release build pipeline: bundle deb/rpm/AppImage (final verification)

## Phase 2 — Frontend `[x]`
- `[x]` Scaffold Vue 3.5 + Vite + TS, shadcn-vue + Tailwind v4, vue-i18n (ru/en), Pinia, TanStack Query, vue-echarts
- `[x]` Unlock / Create Vault screen (password, auto-lock TTL, authentication indicator)
- `[x]` Dashboard: portfolio summary, by-chain/by-store, FX rate, "price updated"
- `[x]` Stores: CRUD, archive, color, jump to holdings
- `[x]` Transactions: list, filters, adding income/expense/transfer/trade, comments, categories
- `[x]` Adder forms with asset autocomplete (`search_assets`), network choice + tx hash (explorer link)
- `[x]` Charts: portfolio dynamics, distribution by assets/networks (ECharts)
- `[x]` Settings: base currency, light/dark/system theme, locale, decimals, password change
- `[x]` Strategy Lab: rule builder (forms + YAML preview), real/synthetic run, equity curve and metrics, paper account
- `[x]` **Wallet page**: sidebar with wallets, calculator (converter + PnL), transaction forms, positions, history, NAV chart

## Phase D — Terminal, Strategy Lab, Bybit import `[~]`

A detailed description of the changes (by file and command) is in [`CHANGELOG.md`](../CHANGELOG.md),
the `[Unreleased]` section. Here — what changed in essence.

- `[x]` **D1. Design system and interface settings** (`docs/ui-design.md`)
  - `[x]` Shared primitives: wallet icons (`src/shared/lib/store-icons.ts`), tooltips
        (`src/shared/ui/tooltip.vue`), spacing scale (`src/shared/lib/spacing.ts`)
  - `[x]` Views (`DashboardView`, `StoresView`, `ChartsView`, `SidebarWallets`,
        `TransactionsView`) moved to shared tokens
  - `[x]` Settings: dashboard widget set, favorite currencies in the header, price
        refresh interval, minimize to tray, "Offline" indicator, a
        "For developers" section (open vault / download JSON)
- `[x]` **D1. Recent vaults** — a list of recent files on the unlock
  screen with marks "exists / not found / old format", a custom path
  through the system dialog, a "Back to list" button
- `[x]` **D2.3. Transaction hash validation** — checking the hash shape
  (`TX_HASH_MALFORMED`) and that it matches the asset's network
  (`TX_HASH_NETWORK_MISMATCH`)
- `[~]` **D2.1. Trading terminal** (`/trading`; the route is temporarily hidden — the chart is moving into an "exchange" type wallet) — a candlestick chart on
  lightweight-charts 5, timeframes 1…1440 min, SMA20 / EMA50 / RSI14, a volume
  histogram (overlaid under the candles), seven drawing tools saved to `localStorage`, live candles
  over Binance WebSocket with reconnect and a Live/Offline indicator
- `[x]` **D2.2. Strategy Lab: trades, signals, forecast** — the engine records an
  explanation for every rule that fired (`SimSignal`), synthetic candles
  are reproducible by `seed`, runs store the generator parameters; a candle chart with
  trade and signal markers, a table of reasons, a GBM forecast fan (`get_gbm_paths`)
- `[x]` **D2.4. Bybit history import with staging confirmation** — trades go
  into `pending_imports` and become transactions **only** after the
  user confirms; deduplication by `execId`, background polling on a schedule
  (`bybit_import_interval_sec`, `0` — off) and the `bybit-import-pending` event
- `[x]` **Misc**: strict `decode_bin` (a truncated bincode snapshot → an error
  instead of silent garbage), `clippy -D warnings` across the whole workspace, 97 new
  localization keys in ru/en

## Phase 3 — Smart growth `[ ]`
- `[ ]` Portfolio report: per period (profit/loss, dividends/staking)
- `[ ]` Category analytics and budgets by category
- `[ ]` Transaction import/export (CSV), templates
- `[ ]` Snapshot backup and vault restore with an integrity check
- `[ ]` Telegram/webhook for price and signals (pull model, no keys in the vault repo)

## Analytics and section simplification
- `[x]` An analytics overview (4 cards) and three sections instead of nine tabs; the conclusion
  in words — facts from the core (`Insight`, `get_report_overview`), text from the locales;
  a jump from the report to transactions (the `category` filter of `TxFilter`)
- `[x]` Removed: the "Graph" tab in transactions, the separate "Queue"/"Runs"/
  "Variables" of schemes, the "Contacts" menu item (now "Transactions → Contacts and debts")
- `[ ]` **Model-generated conclusion (phase 2).** The templated conclusion always stays, the model —
  only when enabled in settings; offline or on error — the template.
  - Only facts (`Insight`) and category names leave the device — not wallet and
    contact names, not individual transaction amounts, not notes.
  - A predefined prompt; the key lives inside the encrypted vault; the response is
    cached per report and period.
  - Two adapters: OpenAI-compatible with presets (OpenRouter, Hugging Face
    Inference Providers, OpenAI, Google Gemini in compatibility mode, Mistral;
    DeepSeek, Qwen/DashScope, Kimi/Moonshot, GLM/Zhipu; local Ollama,
    LM Studio, llama.cpp server; "custom address") and native Anthropic (Claude).
  - The model comes from the preset's list or is entered manually; if the provider serves `/models`,
    the list is fetched.
- `[ ]` **History as a flow.** A graph element = a transaction, showing how
  money moved through wallets and exchanges. The previous graph (`get_tx_graph` is kept
  in the core) was inconvenient: rethink the view before bringing the tab back.
- `[ ]` Overview cards as Home widgets (the `OverviewCard` component is already
  reusable)

## Device sync
- `[x]` Over Wi-Fi: mDNS + Noise, code pairing, field-level merge, cancel (`docs/sync.md`)
- `[ ]` Over Bluetooth (BLE) on top of the same protocol

## Phase 4 — Hardening / polish `[ ]`
- `[ ]` Auto-lock on inactivity, a pause after N failed unlock attempts
- `[ ]` Windows/macOS builds in CI (GitHub Actions + mise steps identical to local ones)
- `[ ]` E2E smoke: create vault → transaction → chart → lock/unlock
- `[ ]` Performance: indexes on transactions, lazy charts on big data
- `[ ]` Updater (tauri-plugin-updater), artifact signing

## Out of scope (deliberately)
- Storing keys in TPM/YubiKey (v1 — password; see docs/security.md)
- Trading orders on exchanges (accounting — yes, execution — no)
