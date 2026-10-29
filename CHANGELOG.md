# Changelog

All notable changes to the project. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows
[Semantic Versioning](https://semver.org/).

The commit list in each section is generated automatically (`mise run changelog`,
`mise run bump`; see [docs/workflow.md](docs/workflow.md#changelog)); the text
above it is the main points for the user, written by hand.

Vault file format: `*.wallet` (an encrypted container), the payload schema is
`VAULT_FORMAT_VERSION` (`crates/core/src/lib.rs`). Backward compatibility of
snapshots is **not guaranteed**: a file written by another schema version will be
rejected with `UNSUPPORTED`.

## [Unreleased]

### Changed — renaming

- **TradeApp → Multi Wallet Tracker (MWT).** The new name in the window, menu and
  packages; the binary and packages are `multi-wallet-tracker` (AUR:
  `multi-wallet-tracker-bin`), identifiers `dev.mwt.desktop`,
  `dev.mwt.app` (Android), `dev.mwt.MultiWalletTracker` (Flatpak). On first
  launch the data and settings folders `dev.tradeapp.desktop`, the paths of recent
  vaults, the reminders key in the system keyring and the autostart entry
  are migrated automatically. The "+ Transaction" binding in Hyprland/Sway has to be fixed:
  the command is now `multi-wallet-tracker --quick-add`. The vault format and the
  export file schemas did not change.

### Changed — incompatible (vault schema)

- **New transaction model.** Instead of 11 kinds there are 4 verbs (Top up,
  Pay, Transfer, Exchange) and 4 special ones (spot, futures, DeFi,
  sale of property). The other side of a transfer or exchange is your own wallet, a contact's
  details or free text. Fees are now deducted from holdings;
  futures are stored as derivatives and are not counted in the portfolio value.
- All persistable types of the next phases (automations,
  budgets, goals, reminders, alerts, utilities, enrichment) were added to the schema in advance, so that the schema breaks
  only once. **A vault created earlier does not open**: the app reports
  `INCOMPATIBLE_VAULT`; there is no converter.
- The commands `add_income`, `add_expense`, `add_payment`, `add_fee`,
  `add_bank_transfer`, `add_card_payment`, `add_wallet_transfer`,
  `add_defi_action`, `add_cash_flow`, `add_trade`, `add_transfer`,
  `update_comment`, `set_tx_prices`, `add_category` were removed; they are replaced by
  `add_transaction` and `update_transaction` with `TransactionInput`.

### Added

- Editing a whole transaction (before, only the comment).
- A transaction list with a filter: wallet (with or without subaccounts), kind, contact,
  period, search, "hide internal"; the same filter in the wallet history.
- The "Transactions → Graph" tab: a lane for each wallet, "next on the wallet"
  links, transfers and exchanges between wallets, and chains;
  collapsing subaccounts, chain highlighting, a click opens the transaction panel.
- The "+ Transaction" button in the header, with a wallet choice, from any section, and
  a system keyboard shortcut for it (off by default, Settings →
  "Quick add").
- In the calendar a click on a day's transaction opens the transaction panel.
- Checks V1–V14 when a transaction is recorded and auto-categorization by rules.
- Contacts with details, turnover per contact and personal debts ("owed to me /
  I owe"); the "Directory → Contacts" section.
- Categories with groups (income, mandatory, daily, savings,
  service) and a default set; managing categories and rules in
  Settings.
- "Property" wallets (with a manual valuation) and "Liability" wallets (loan, mortgage,
  credit card); net worth on the Dashboard; the "Count in budget" flag.
- Savings percentage, budget currency and day-budget mode in the vault settings.
- Subaccounts for spot and futures trades imported from Bybit.
- A first-steps checklist on the Dashboard.
- Utilities: metered bills, fixed ones and rent with a deposit; readings,
  trend, forecast and an anomaly mark; "Record expense" opens a ready
  payment; the "Fixed expenses" widget on the Dashboard.
- Transaction losses (fee + difference from the market rate), transaction chains
  ("gave → received" with the loss per link), the "Losses and chains" tabs in
  transactions and "Losses" in analytics, a loss column in the list.
- Transaction panel: amounts, rate, loss, chain, "Continue chain",
  "Link to…", editing and deletion.
- Directory → Rates: a table of rates to the base currency and a converter for a date.
- Checking transactions on the blockchain by hash (network fee, status), only with
  the user's consent; what is found is applied manually.
- The "Analytics" section: cash flow, annual summary, category matrix,
  net worth over time, wallets, PnL, portfolio and inflation; a shared filter,
  an "in today's money" mode and CSV export. The "Charts" page became
  the "Portfolio" tab.
- A spending calendar in "Transactions", the widgets "Spending for 7 days", mini-calendar and
  "Net worth" on the Dashboard, reference inflation in settings.
- The "Planning" section: automations with a balance check before a
  transaction is created (a rejected run waits for "Retry / Skip"), spending limits,
  goals with distribution of a shared wallet, "needed per month", realism,
  forecast and a savings calendar, reminders and price alerts with
  system notifications.
- Day budget in two modes ("Month with carry-over" and "Until payday"), month
  totals and "Upcoming" on the Dashboard; a warning about exceeding a limit in the
  transaction form; a time zone in settings.
- History of currency rates and asset prices: a refresh no longer overwrites the previous
  rate. On save a transaction gets the price at the rate of its date (under the
  "Price" field you can see which day); transactions "without a price" are re-priced on the next
  rate refresh. The `convert_amount` command converts an amount between currencies
  for a date.

### Fixed

- Backend error codes were lost on the frontend: `call()` throws an already parsed
  error, while `parseError` understood only a string, so instead of a clear text
  "Unknown error" was shown.
- A day's transactions in the calendar are selected by the time zone from settings, like
  the calendar cells (before, by the browser's zone).
- The frontend called `exchange_key_status` and `clear_exchange_key` without the
  `input` object, and the key status in the transaction form was not loaded at all, so the latest
  Bybit trades did not appear in the form.
- Realized PnL no longer counts a fee as profit and no longer counts
  transfers between your own wallets as a sale.
- Fiat holdings (RUB, THB, …) are no longer worth 0; they are valued at the
  currency rate.
- Transaction changes, import and revaluation now update the day budget, limits,
  goals and reports immediately, without a reload.
- Cost basis, turnover per contact and the value chart in the base currency
  are computed at the rate on the transaction date, not today's.

Phase D: a trading terminal, Strategy Lab with signal explanations, Bybit
history import with staging confirmation, as well as a pass over the design system,
transaction hash validation and a list of recent vaults.

### Added

#### Trading terminal (D2.1)

- `src/components/charts/TradingChart.vue`: a candlestick terminal on
  lightweight-charts 5: timeframes 1/3/5/15/30/60/240/1440 min, indicators
  SMA20 / EMA50 / RSI14 (RSI in a separate pane), a volume pane, an OHLC
  legend, a symbol watermark, a "Live / Connecting / Offline" indicator.
- Drawing tools: trend line, horizontal line, Fibonacci levels,
  rectangle, brush, text, arrow; "magnet" mode, undo, clear.
  Drawings are saved in `localStorage` under the key
  `tradeapp.drawings.<SYMBOL>` and survive a restart.
- Live data: a direct connection to the Binance WebSocket
  (`wss://stream.binance.com:9443/ws/<pair>@kline_<tf>`) with reconnect;
  on a drop the UI shows "Offline" and keeps working on the last
  loaded candles.
- `src/views/TradingView.vue`: the terminal screen: a symbol field and a quick
  pick from a list (BTC, ETH, SOL, BNB, XRP, DOGE, ADA, AVAX).
- The `/trading` route, an item in the side navigation and in the tray.
- A new command `get_candles` (`GetCandlesInput` → `GetCandlesOutput`),
  a thin wrapper over `sim::engine::fetch_candles`, works without an unlocked
  vault.
- Dependencies: `lightweight-charts@5.2.1`, `lightweight-charts-drawing@0.4.0`.

#### Strategy Lab: trade chart, signals and GBM forecast (D2.2)

- `SimRun` was extended with the fields `signals`, `candle_params`, `granularity_min`.
- `SimSignal` is a fired signal: time, side (`entry` / `exit`), reason
  (`rules` / `stop_loss` / `take_profit` / `liquidation`), a text explanation
  of the rule that fired, and the price.
- Signals are written right in the engine: `run_on_candles` now explains every
  trigger (for example `sma20(25.3100) gt 25`) instead of just executing it.
- Synthetic candles became reproducible: `synthetic_candles` takes
  `seed` and `step_min`, uses `StdRng` and a fixed epoch
  (2026-01-01T00:00:00Z). The same seed gives the same series, so the trade and
  signal markers match the chart when it is reopened.
- `CandleParams` are the generator parameters (`seed`, `n`, `start_price`,
  `annual_vol`, `candles_per_year`), stored in the run as a replay key.
- A new engine API: `synthetic_from_params`, `realized_sigma` (σ from
  log returns), `gbm_paths` (zero-drift paths),
  `fetch_candles_between` (a historical range with pagination),
  `rfc3339_to_millis`, `iso_from_millis`.
- New commands: `get_run_candles` (restores the run's candles: synthetic ones
  by seed, real ones by fetching the range from Binance) and `get_gbm_paths`
  (a forecast fan from the run's last price; σ is taken from `candle_params`
  or from realized vol on real candles).
- `src/views/StrategyLabView.vue`: run selection, a candlestick chart with markers
  of buys/sells and entry/exit markers, a table of signals with reasons,
  a GBM "fan" toggle, a separate equity curve chart.
- `echarts.ts` now registers `CandlestickChart`, `ScatterChart` and
  `DataZoomComponent`.

#### Bybit history import with staging confirmation (D2.4)

- `PendingImport` in `VaultDoc.pending_imports` is a trade awaiting
  confirmation. It stores both legs, `execId` (the deduplication key), the fee and
  the time in ISO-8601; money fields are `Amount`.
- `PendingImport::from_execution` turns a Bybit fill into a staging record and
  returns `None` for unusable trades: without `execId`, with an unreadable
  quantity or price, with a zero quantity. Garbage does not reach the
  user's transactions.
- `UnlockedVault` methods: `stage_imports` (idempotent by `execId`,
  limit 10,000), `pending_imports` (newest first), `confirm_pending_imports`
  (turns staging into trades: two legs, like `add_trade`, with a shared
  `group_id`), `discard_pending_imports`.
- Commands: `import_bybit_history`, `list_pending_imports`,
  `confirm_pending_imports`, `discard_pending_imports`.
- `Execution.exec_id` + `Execution::category_hint()`; `BybitClient::all_executions`
  paginates history in 7-day windows (`/v5/execution/list` does not support
  `cursor` and forbids windows wider than 7 days), up to 12 pages per category.
- Background check: a thread in `crates/app/src/lib.rs` wakes up every 30 s,
  reads `settings.bybit_import_interval_sec` (`0` is off, otherwise
  clamped to 60…86400) and puts new trades into staging. The event
  `bybit-import-pending` ({ staged }) shows a banner in the header.
- UI: in the Bybit card on the wallet screen there is a "Load history" button,
  a counter of pending ones, a table with rows and "confirm" /
  "reject" actions per trade and for all at once.

**Invariant:** no command and no background poll creates transactions
by itself, only an explicit user confirmation.

#### Design system and interface settings (D1)

- `src/lib/store-icons.ts`: icons by wallet type (used in
  `SidebarWallets`, `StoresView`, `WalletView`), `src/components/ui/tooltip.vue`,
  a tooltip primitive (AppShell, terminal), `src/lib/spacing.ts`, a spacing
  scale (a stub for the next layout pass).
- `src/views/DashboardView.vue`, `StoresView.vue`, `ChartsView.vue`,
  `SidebarWallets.vue`, `TransactionsView.vue` were moved to shared tokens.
- Configuring the dashboard widget set (`settings.dashboard_widgets`), favorite
  currencies in the header, price refresh interval, minimizing to the tray, an
  "Offline" indicator.
- A "For developers" section in settings: open the vault and download its JSON.
- `docs/ui-design.md`: the design system rules; the skill
  `.opencode/skills/ui-design-system`.

#### Recent vaults

- The commands `recent_vaults`, `select_vault`, `forget_recent_vault`,
  `pick_vault_file` and a list of recent files in the unlock screen.
- `UnlockView` has two steps: a choice from the list of recent ones (marked "exists" /
  "file not found" / "old format") or your own path with a system dialog;
  a "Back to the list" button.
- An empty password is rejected on input (`INVALID_INPUT`); otherwise the file
  would be created effectively unencrypted given an empty password.

#### Transaction hash validation (D2.3)

- New error codes `TX_HASH_MALFORMED` and `TX_HASH_NETWORK_MISMATCH`:
  the shape of the hash and its match with the asset's network are checked, not just
  that it is non-empty.

### Changed

- `storage::decode` and `decode_bin` no longer silently accept truncated
  bincode snapshots: `decode_bin` checks that the payload is fully consumed
  (`n == bytes.len()`), otherwise `DESERIALIZE`. The schema version was not bumped,
  there is no v1→v2 migration: an incompatible file is rejected explicitly.
- `sim::engine::run_on_candles`: rules are evaluated through `rules_detail`,
  which returns a description of the matched conditions; stop-loss and
  take-profit take priority if both conditions hold on one candle.
- `rules_detail` requires a previous indicator value, so the
  behavior during indicator "warm-up" did not change.
- The money fields of `PendingImport` are `Amount`, not strings; the staging limit is 10,000 records.
- `ui-design-system`, `docs/ui-design.md`, the plan for storage and exchanges:
  documentation in the repository.
- The types `PendingImport`, `SimSignal`, `CandleParams`,
  `GetRunCandles*`, `GbmPath*`, `ImportBybitHistory*`, `Confirm/DiscardPendingImports*`,
  `RecentVault`, `DevDumpVaultOutput`, `BybitExecution*` were added to `ipc.gen.d.ts`.

### Fixed

- `clippy -D warnings` across the whole workspace: `collapsible_if` in
  `service.rs` was converted to let chains, unused imports were removed in
  `storage/mod.rs`, `vault_recents.rs`, `settings.rs`.
- `fetch_candles_between` duplicated neighboring candles at page boundaries;
  deduplication by `ts` was added.
- Dead `cursor` pagination code in `all_executions` was replaced with
  window pagination (Bybit ignores `cursor` for the executions list, so
  pages would have repeated until the limit was reached).

### Other

- The tray and side navigation gained the items "Terminal" and "Lab".
- All user-facing texts were added to both locales (ru/en): 97 new
  keys, the key sets are synchronized.
- `docs/ipc.md` was extended with sections on run replay and Bybit import;
  `docs/data-model.md` with the new entities.

### Checks

`cargo fmt`, `cargo clippy --workspace -- -D warnings`, `mise run check`
(eslint + vue-tsc + 81 workspace tests), `pnpm exec vite build`: green.
