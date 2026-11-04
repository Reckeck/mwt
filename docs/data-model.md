# Data model

All entities live in **a single encrypted file** (the vault). A "store" is a
portfolio container, not an asset; an asset is represented by a `Holding` inside a store.

## VaultDoc (the document root)

| Field | Type | Comment |
|---|---|---|
| `version` | `u32` | `VAULT_FORMAT_VERSION = 3`. A file of an older version opens and migrates (`storage::decode_versioned`): an untouched copy is placed next to it as `<vault>.v<N>.bak`, then the document is rewritten in the current version. A version from the future is `UNSUPPORTED`; a file of a known version that does not fit its schema is `INCOMPATIBLE_VAULT`. Details — the "Versions and migrations" section below |
| `id` | `String` | uuid |
| `created_at` | `String` | RFC3339 UTC |
| `settings` | `Settings` | vault-scoped: decimals, widgets, intervals, tray |
| `stores` | `Vec<Store>` | portfolio containers |
| `holdings` | `Vec<Holding>` | derived from transactions (`recompute_holdings`) |
| `transactions` | `Vec<Transaction>` | transaction history (the source of truth) |
| `categories` | `Vec<Category>` | categories with groups; a new vault gets a default set (`default_categories()`) |
| `price_cache` | `Vec<PricePoint>` | asset → USD rate, with a TTL |
| `fx_cache` | `Vec<FxPoint>` | units per 1 USD for currencies |
| `exchanges` | `Exchanges` | exchange keys (see below); not exported to TS |
| `pending_imports` | `Vec<PendingImport>` | Bybit trades awaiting confirmation |
| `contacts` | `Vec<Contact>` | external parties and their requisites |
| `categorization_rules` | `Vec<CategorizationRule>` | "text → category" |
| `manual_prices` | `Vec<ManualPrice>` | a manual valuation of property |
| `envelopes` | `Vec<Envelope>` | envelopes (see above) |
| `debt_terms` | `Vec<DebtTerms>` | the term and note of a debt with a contact per asset; the debt amount comes from transactions with `debt` |
| `shared_events` | `Vec<SharedEvent>` | `serde(default)`: shared spending (see "Shared events"); shares, balances and status are not stored |
| `budgets`, `goals`, `reminders`, `price_alerts`, `utility_items`, `meter_readings`, `enrichments` | `Vec<…>` | planning. Since v3 automations are schemes (`workflows`) with `Workflow.automation: AutomationSpec { preset, schedule, starts_at, template, amount_mode (fixed/varies), workday_shift (none/back/forward) }`: the graph is built from a form (`workflow::automation`), runs are `workflow_runs`; `Automation`/`AutomationRun` are only IPC views. `preset` is a ready wallet template (salary, DCA, loan payment…); `UtilityItem.property_id` — utilities always belong to a property; `PriceAlert.store_id` (`serde(default)`) — the exchange whose terminal holds the alert, `None` — common. v2: `Reminder.repeat` (every N days/weeks/months/years), `Reminder.source` (an auto-reminder and its source), `muted`, `user_edited`, `snoozed_until`; `MeterReading.day` (day of month, `None` = month end) |
| `notification_log` | `Vec<NoticeRecord>` | v2: shown notifications (a "source + occurrence" key, so as not to repeat after a restart), at most 500 |
| `sync` | `SyncState` | `serde(default)`: change stamps, tombstones, trusted devices, batches for undo, vault transfers to new devices (`transfers`) — device sync (`docs/sync.md`); not exported to TS, a copy from the JSON editor is ignored |

### Settings — vault-scoped only

`display_decimals` (8 by default), `dashboard_widgets` (a 12-column grid:
`DashboardWidgetCfg { id, col, row, width, height, pinned, store_id? }`; a widget not in the list
is not on the dashboard; `pinned` — the title is hidden, position and size are pinned),
`fx_refresh_interval_sec` (300), `minimize_to_tray` (true) — these three moved to
`app_settings.json` with device sync (the first open
carries the values over; the fields stay in the vault but are not read), `savings_rate` (the share of income
for savings, 0..1, `0.10` by default), `daily_budget_mode` (`month` —
a month carrying over the remainder, the default; `until_income` — until the next
income), `budget_currency` (empty = the base currency), `notifications` (v2:
system notifications on/off, hide details while the vault is locked,
the time of auto-reminders, per category — on/off, how many days ahead, whether to create
auto-reminders), `inflation_manual` (flat annual inflation by lowercase currency, only where the group differs from the shipped table; group data), `inflation_adopted` (device flag: this device copied its old app-config rates into `inflation_manual`), `crypto` (direction: whether the vault accounts for cryptocurrencies;
`false` hides `crypto`/`exchange` wallets, exchange widgets, and
coins in the currency and asset pickers in the interface and stops background exchange polling; data and
balances remain in the totals; the field is absent in an old or imported vault, then
`open` derives it in memory — `true` if there is a non-archived crypto wallet or
exchange — and writes it on the next save).

Settings that describe the **user**, not this vault, live not in the document
but next to the app — see "App settings" below.

## App settings (outside the vault)

The theme, language, base currency and favorite currencies follow the user, not the
vault: they survive opening another vault and are read **before** unlock
(so the sign-in screen renders in the right language at once). So they are not
encrypted and not duplicated in each vault — they live as plain JSON in
`app_config_dir`.

`crates/core/src/appconfig.rs`, two files:

| File | What |
|---|---|
| `app_settings.json` | `AppSettings { theme, locale, base_currency, favorite_currencies[], …, device_id, device_name, sync { enabled, confirm_first, prefer_main }, minimize_to_tray, fx_refresh_interval_sec, display_decimals }` — `device_id` is created once per installation and never enters the vault |
| `currencies.json` | the currency catalog cache, `SupportedCurrencies { fiat[], crypto[] }` |

Defaults: `theme: auto`, `locale: auto`, `base_currency: "usd"`, favorites empty.
`base_currency_chosen` — whether the base currency was chosen: on a fresh install it is `false`, and
the vault create or open form asks for a currency; any write of `base_currency`
sets `true`; a settings file without the field (an installation older than the field) is read as `true`.
A broken or missing file gives defaults — the sign-in screen must always
render.

**Normalization on write:** `base_currency` → trim + lowercase, empty → `usd`;
favorite currency codes → uppercase, exactly 3 letters, deduplicated.

### Currency catalog: the degradation ladder

`get_supported_currencies` never fails — the first available rung is chosen:

1. **network** — `open.er-api.com` (fiat) + CoinGecko `/coins/list` (crypto);
   a successful response is cached in `currencies.json`;
2. **cache** — the last successful response if the network is unavailable (a failed
   half is taken from the cache, not dropped);
3. **built-in list** — `FALLBACK_FIAT` / `FALLBACK_CRYPTO`, if there has
   not been a hit yet.

`notice` in the response says which rung was used: `builtin`, `cached` or
`null` (answered just now). The UI shows a warning based on it that the list
may be incomplete.

### Base currency — a runtime field, not a document one

`UnlockedVault::base_currency` lives in memory, not in `doc`: the base currency
is configured by the app, so the vault cannot know about it. After
`open`/`create` the IPC layer calls `sync_base_currency`, which pushes the
value into the vault; changing the base currency recomputes holdings and average
costs (`set_base_currency` → `recompute_holdings`), so that the figures on screen
match the label. The `price_base` stamps already recorded in transactions are not
rewritten — they stay as they were fixed at the moment of the deal.

## Store / StoreKind

`Store { id, name, kind, color, archived, note, parent_id?, sub_type?, binding, purpose, contact_id?, liability?, custody?, property?, secured_by?, exchange?, loyalty?, deposit? }`.

`kind` is the wallet type, by which the sidebar is grouped and settings chosen:

| Type | What it is | Roots | Sub-accounts (`sub_type`) | Default purpose |
|---|---|---|---|---|
| `cash` | cash | exactly one, created with the vault, not archived or deleted (`CASH_LOCKED`) | `pocket` | `everyday` |
| `bank` | bank | any number | `card` ("card / account" — the same for accounting), `deposit`, `savings`, `pds`, `credit_card`; the former `account` stays valid. A new bank is created at once with a first card (`create_bank_with_card`), the opening balance goes to it | `everyday` |
| `crypto` | one's own wallet (`custody: hot \| cold`) | any number | `token` | `investment` |
| `exchange` | an exchange (`exchange: ExchangeProfile`) | any number | `funding`, `unified`, `earn`; the former `spot`, `futures`, `pair` stay valid but are not offered for new ones (`StoreKind::admits_sub_type`). Spot and futures are kept on `unified`: the transaction form and the import put a trade there | `investment` |
| `property` | property (`property: PropertyInfo`, `utilities_pay_day?`) | any number | — | `investment` |
| `loyalty` | a bonus program (`loyalty: LoyaltyInfo { points_symbol, point_value, currency, expires_at?, expiring_points? }`) | any number | — | `savings` (in net worth — non-financial assets) |
| `liability` | a loan (`liability: LiabilityTerms`, `secured_by` — the pledged property) | any number | — | — (never part of the budget) |

The rule catalog is served by `get_wallet_catalog` (`WalletCatalog::build` in `ipc_contract.rs`) —
the frontend does not duplicate it.

- `purpose: everyday | savings | investment | transit` replaced the `in_budget` flag:
  only `everyday` enters the daily budget (`Store::in_budget()`); `transit` is
  not in net worth either. A sub-account takes the purpose of its template (a deposit —
  `savings`), otherwise the parent's.
- `binding` — a sub-account's strict binding: `asset(AssetRef)` (a currency or token),
  `pair { base, quote }` (both coins, exchange in both directions), `market(spot | futures | earn)`.
  A root has `any`. A transaction with a foreign asset gets `BINDING_MISMATCH` (V15);
  changing the binding is rejected if the posted transactions do not fit into it.
- `deposit: DepositTerms { rates[{ from, rate }], capitalization, opened_at?, term_months?, auto_accrue, pds? }` —
  on `savings`/`deposit`/`pds` sub-accounts; rates are a history, interest is computed by day
  on the actual balance (`service/interest.rs`); `pds: PdsTerms { first_contribution_at, income_bracket }`
  — PDS co-financing (up to 36,000 RUB a year, 10 years). `VaultDoc.key_rates` — a cache of the Bank of Russia key rate.
- `market(funding | unified)` — Bybit accounts: "Funding" (coins only) and "Unified
  trading" (coins and derivatives); the import distributes trades, transfers and Earn among them.
- `TxMeta.utility_item_id?` — a payment is tied to a utilities bill (old ones are matched by
  comment and category); `KindDetails.for_month?` — the month `YYYY-MM` the income is for, if it is not the month of the date (a salary on the 5th is for the previous one); `KindDetails.source_store_id?` — the card whose purchases
  the cashback or points came for.
- `liability: LiabilityTerms { kind, rate?, limit?, due_day?, monthly_payment? }` —
  on `liability` and on the `credit_card` sub-account; `limit` bounds the credit card for automation.
- `exchange: ExchangeProfile { provider, import_interval_sec, terminal: TerminalState }` —
  the exchange, its background import and **the entire terminal** (watchlist, active symbol,
  timeframe, indicators, drawings per symbol ≤ 256 KB, layout). Keys are separate,
  in `VaultDoc.exchanges`, and are set only after `provider` is chosen (`PROVIDER_REQUIRED`).
- `contact_id` — whose account this is (a bank, an exchanger); only on a root (R10).

### The tree: wallet → sub-account

Changing a wallet's kind, moving an account to a wallet of another kind, splitting out and merging accounts — `service/store_change.rs` (`plan_store_change` / `apply_store_change`, see `docs/ipc.md`).

`parent_id = None` — a wallet (root), otherwise a sub-account. `MAX_STORE_DEPTH = 1`:
**two levels**. A sub-account is always of the same type as its parent and must have a
`sub_type` from the catalog and a binding of the right shape. A node with sub-accounts also has its own
funds and transactions ("Main" in the UI).

The rules are checked on write (`validate_placement`, `check_sub_account` in
`service/mod.rs`); reading is tolerant.

| # | Rule | Error code |
|---|---|---|
| R1 | The parent exists | `MISSING` |
| R2 | A node is not nested into itself | `STORE_CYCLE` |
| R3 | A sub-account has no sub-accounts | `STORE_TOO_DEEP` |
| R4 | The parent is not archived | `STORE_PARENT_ARCHIVED` |
| R5 | The name is unique among siblings (ignoring case and edge spaces); roots — among roots | `STORE_NAME_TAKEN` |
| R6 | A root does not become a sub-account; a sub-account does not become a root | `STORE_ROOT_MOVE` / `WALLET_NOT_APPLICABLE` |
| R7 | A sub-account is moved only to a wallet of its type | `SUB_TYPE_NOT_ALLOWED` |
| R8 | Archiving cascades down; restoring does too, but not under an archived parent | `STORE_RESTORE_PARENT_FIRST` |
| R9 | Deletion only without transactions and without sub-accounts; cash — never | `INVALID_INPUT` / `CASH_LOCKED` |

### Envelopes

`VaultDoc.envelopes: Vec<Envelope { id, store_id, name, asset, allocated, moves, target?, goal_id?, created_at }>` —
a virtual marking of the balance: money does not move, part of the asset's remainder in
the wallet is marked as taken. Money can be set aside only from the free
remainder (`INSUFFICIENT_FUNDS`); a spend dipping into envelopes is allowed, but the
preview warns (`PreviewWarning::IntoEnvelopes`), and the envelope
is shown as `short`. A goal linked to an envelope counts progress from it.
`moves: Vec<EnvelopeMove { id, delta, at }>` is the journal of changes (additive,
`#[serde(default)]`); `allocated` is always its sum and is written only through
`Envelope::record_move`, so top-ups made on two devices both count after sync. An envelope
from before the journal gets the entry `open:<envelope id>` on open.

### StoreScope — the report area

`StoreScope { store_ids: Vec<String>, include_children: bool }` — a single filter
by wallets: an empty list = the whole vault, `include_children` (`true` by default)
expands each id into its subtree. The transaction list, the value history
and the node's realized PnL are built on it. It is not yet stored in the vault.

## AssetRef

`{ symbol, name, coingecko_id?, chain?, contract?, quote }`.
`chain` is a `Chain` variant (`bitcoin`, `ethereum`, `bsc`, `solana`, …, `fiat`).
For fiat, `AssetRef::fiat(symbol, name)` is built with `chain: Fiat`.
`AssetRef::key()` is the canonical key for caches and comparisons
(used in `PricePoint.asset_key`).

## Transaction (the source of truth)

`Transaction` is a **closed enum** with an external tag (bincode cannot do internal
tags). Four verbs and four special kinds:

```rust
enum Transaction {
    Topup(TopupTx),       // Top up
    Payment(PaymentTx),   // Pay
    Transfer(TransferTx), // Transfer (one asset)
    Convert(ConvertTx),   // Convert (different assets)
    TradeSpot(TradeSpotTx), TradeFutures(TradeFuturesTx),
    Defi(DefiTx), AssetSell(AssetSellTx),
    Adjustment(AdjustmentTx), // Correction: a reconciliation with reality
}
```

| Structure | Fields |
|---|---|
| `TxMeta` | `id`, `timestamp` (RFC3339 UTC), `comment`, `categories` (keys), `flow_id?` (chain), `contact_id?` ("with whom / through whom"), `origin` (`manual` \| `import { exec_id }` \| `automation { automation_id, planned_at }`) |
| `PriceStamp` | `price_usd?`, `price_base?`, `price_base_currency?` (lowercase code `price_base` is in; absent on old records, then read as before), `fx_override?` — on each valued side |
| `Fee` | `{ asset, amount, price }` — instead of the `fee` + `fee_asset` pair |
| `AccountRef` | `store(id)` \| `contact { contact_id, account_id? }` \| `raw(text)` — one side of a movement |
| `TopupTx` / `PaymentTx` | `store_id`, `asset`, `amount`, `fee?`, `from?` / `to?` (not `store`), `details: KindDetails`, `price` |
| `TransferTx` | `from`, `to` (≥ 1 side is a `store`), `asset`, `amount` (the recipient gets exactly this), `fee?` (paid by the sender), `network?`, `tx_hash?`, `debt?: DebtEffect`, `price` |
| `ConvertTx` | `from`/`to: ConvertSide { account, asset, amount, price }`, `fee?`, `rate_override?` |
| `TradeSpotTx` | `store_id`, `sold`/`bought: TradeSide { asset, qty, price }`, `fee?`, `liquidity?`, `order_id?` |
| `TradeFuturesTx` | `store_id`, `contract` (the derivative), `settle`, `direction` (long/short), `action` (open/close), `quantity`, `fill_price`, `leverage?`, `fee?`, `closed_pnl?` (close only), `order_id?`, `category` (linear/inverse), `price` (the settle price) |
| `DefiTx` | `store_id`, `action` (stake/unstake/claim/add_liquidity/remove_liquidity), `asset`, `amount`, `protocol?`, `pool_id?`, `network?`, `tx_hash?`, `fee?`, `price` |
| `AssetSellTx` | `store_id`, `asset`, `unit_price`, `quantity`, `currency`, `proceeds_to?` (a node), `fee?`, `price` |
| `AdjustmentTx` | `store_id`, `lines: AdjustmentLine { asset, before, after, price }`, `fee` (always `None`). `before` is the accounting balance at the moment of the transaction without itself, computed on every save; lines with no difference are dropped. `after` (the entered balance) is immutable: on any history edit (`recompute_holdings` → `rebase_adjustments`) the `before` of all corrections is recomputed in time order, so the difference follows the history; a line whose difference became zero stays. Legs are `LegRole::Adjustment` on `after − before`: they move the balance and net worth, but not income/spending, the budget, categories or realized PnL (reducing a position "sells" nothing). No categories; on all wallet kinds except property |
| `KindDetails` | `cash_breakdown?` (`cash` only, Σ = the amount), `unit_price?` (`asset` only: the unit price in the base currency on the purchase date), `network?`, `tx_hash?` (crypto wallets only) |

### Legs (Leg)

`Transaction::legs()` expands a record into `Leg { store_id, asset, amount (signed), role, to_store_id?, price_* }`. Legs are **not stored**; everything that sums
money works only with them. Only `store` sides have legs: a transfer to a contact gives one leg.

`role`: `principal` — the movement itself; `fee` — a fee, a separate outgoing leg
(earlier fees were recorded and ignored, so holdings were overstated);
`pnl` — the realized PnL of closing a future, in the settle currency.

Derivatives are `AssetRef::perp(exchange, symbol)`, key `perp:{exchange}:{symbol}`. A position
lies as a holding of the contract but is **not part** of value, net worth and history;
on the wallet page there is an "Open positions" block.

Realized PnL: a sale is only an outgoing `principal` leg with a known
price and without a second node of its own; `fee` goes as a separate sum (`fees_base`),
`pnl` — directly.

### Mapping kinds to the wallet type

`allowed_kinds(StoreKind)` and `default_kind(StoreKind)` in `model/transaction.rs` are
the only source; the frontend asks `transaction_kinds_for_store_kind`.

| Type | Verbs | Special | Default |
|---|---|---|---|
| `cash`, `bank` | all four | — | Pay |
| `exchange` | all four | spot, futures | spot |
| `crypto` | all four | DeFi | Transfer |
| `property` | Top up ("Acquire") | sale | Top up |
| `liability` | Pay, Transfer | — | Transfer |

### Validation (V1–V14)

Checked on every `add_transaction` / `update_transaction`
(`service/validate.rs`); an error is `INVALID_INPUT` with a text for the form.
Nodes exist and are not archived; the kind is allowed for each node; `cash`/`bank`
hold only fiat, `asset` — only things; amounts > 0; Top up/Pay have
a category of a fitting group (Top up — income or bookkeeping, Pay — not
income), not archived; a transfer and a conversion have ≥ 1 own side and different sides,
a conversion has different assets; a contact requisite belongs to it and is active; details are
only those allowed for the type; `closed_pnl` — only on close; the date is valid and
not more than a day ahead; the debt direction matches the sides. A negative
balance (V13) is not an error.

Before the check, categories are normalized, and empty ones are filled by the first
matching rule (`CategorizationRule`: a substring of the comment or contact name,
case-insensitive).

## Contacts

`Contact { id, name, kind (person|company|exchanger), accounts, note, archived, created_at }`,
`ContactAccount { id, group?, label, kind (iban|account|crypto_address|card|phone|url|other), value, currency?, network?, active }`.
An exchanger is a contact of kind `exchanger`, an exchange through it is a `Convert` with `meta.contact_id`.

- Deleting a contact referenced by transactions or wallets is an error
  ("archive it"). A removed requisite that had transactions is not deleted
  but becomes `active: false`.
- Turnover (`contact_turnover`) — sent / received by asset, without internal
  movements.
- Debts are `TransferTx.debt`: `lend` (lent), `collect` (got back),
  `borrow` (borrowed), `repay` (repaid). `contact_balances()` — "owed to me / I
  owe" per contact and asset. Debts are neither income nor spending.

## Shared events

`SharedEvent { id, title, created_at, note, archived, starts_on?, ends_on?, currency, rounding_step, category_id?, organizer, i_participate, participants, items, settlements, adjustments }`
(`model/shared_event.rs`). Dates are `YYYY-MM-DD` strings (the end is not before the start, an end equal
to the start collapses to one day). `organizer`, `Purchase.buyer`, `Settlement.participant_id`,
`ShareOverride.participant_id` are `"me"` (the vault owner) or a participant id.

| Structure | Fields |
|---|---|
| `Participant` | `id`, `name`, `contact_id?` (none — a guest), `details?: GuestDetails { phone, messenger, requisites: ContactAccount[], note }` — for a guest only |
| `EventItem` | `id`, `title`, `unit`, `category_id?`, `plan?: ItemPlan { qty, price, price_mode }`, `purchases`, `cancelled`, `split: Equal { among } \| ByUnits { units }` (an empty `among` — everyone) |
| `Purchase` | `id`, `buyer`, `qty`, `amount` (per unit or total — `price_mode`), `currency`, `rate_to_event` (on the purchase date, not recomputed), `rate_manual`, `tx_id?` (my `Payment`), `tx_owned` (the event created the payment, so it will delete it), `at` |
| `Settlement` | `id`, `kind` (transfer \| offset \| write_off \| reassign), `participant_id`, `direction`, `amount`, `at`, `tx_id?` (a transfer with `DebtEffect`), `to` (who took it, for reassign) |

The schema is additive (`serde(default)`), `VAULT_FORMAT_VERSION` did not change; an old vault without
`shared_events` is read in JSON and MessagePack (the test `an_old_vault_without_shared_events…`).
Sync: the collection `records("shared_events", ID)`. `removed_with_payment` (backend only) keeps what left an event
with a deleted payment, until the payment is back or gone for good (`docs/sync/compatibility.md`).

**Transactions are not marked.** The "payment/transfer → event" link lives only in the event
(`Purchase.tx_id`, `Settlement.tx_id`), not in `TxMeta`: v1 bincode files are read by field
position, and a new field in `TxMeta` would break them (the same argument as for `TxOrigin::Workflow.estimated`).
The reverse index is built by `shared_tx_links()`. If a transaction is deleted bypassing the event, the purchase/settlement
stays with `payment_missing`.

What is computed (`service/shared_events/split.rs`), all in the event currency:
- a purchase = `amount (× qty when per_unit) × rate_to_event`, rounded to cents;
- an item's share is computed exactly, to the cent: equally among the circle or by units, the cent remainder
  is taken by the item's buyer (the first largest purchase) if they share the item, otherwise the first in
  the circle;
- **each person's total is rounded to `rounding_step` once** (not per item); the rounding remainder
  is taken by the organizer if they participate, otherwise whoever paid more (on a tie —
  the first in the list), then by descending share; a share never goes below zero (what did not fit
  passes to the next). The sum of shares equals the sum of purchases to the cent;
- a manual total (`adjustments`) pins a person (they are not rounded), the remainder is split by the others
  in proportion. The sum of manual totals is not more than what was spent, and if everyone is pinned — it equals it;
  otherwise (after a purchase is deleted) all manual totals are ignored and `SharedEventView.overrides_invalid`
  = true;
- `reassign` reduces the debtor's share and increases the shares of `to` equally (by default everyone who shares
  items: not me if I do not participate);
- a person's balance = share − paid − given to the lead − written off; the lead's balance is minus the sum of
  the others (all settlements go through them);
- status: no purchases — `planning`; purchased and all balances settled — `closed` (no settlements needed);
  otherwise without settlements — `in_progress`, with settlements — `settling`;
- a contact's debt (`contact_balances`, `contact_balances_until`) = transfers with `debt` **plus** the balance of
  events involving me at the same moment (purchases and settlements no later than it); the event's transfers are never
  part of the first term; events without my participation are invisible;
- analytics: the event's payment counts toward my share of the purchase, someone else's purchase is my share from the purchase date,
  a write-off is spending in the event's category, returns are not counted as spending.

Link with the journal: deleting a linked transaction removes the purchase/settlement, an edit of the amount and date is carried
into the event (see `docs/ipc.md`). Deleting an event leaves all transactions ordinary. Any edit of an
event clears its `archived`.

## Categories

`Category { key, name?, group, archived }`. `key` is immutable and stored in
transactions; `name: None` — the label comes from i18n `categories.default.<key>`. The groups:
`income`, `fixed` (mandatory), `daily` (everyday), `savings`, `service`
(bookkeeping — outside budgets). Only an unused category can be deleted.

## Net worth, manual price, classification

- `net_worth()` → `NetWorth { financial, non_financial, receivables, liabilities, payables, net }`
  in the base currency; `PortfolioSummary` carries `assets_base`, `liabilities_base`,
  `net_worth_base`.
- `ManualPrice { asset_key, unit_price, currency, updated_at }` — a valuation of a thing in
  an `asset` node, applies to the whole history (D24). Without it — the purchase price.
- `classify_flow(tx) → FlowClass` (`income`, `fixed`, `daily`, `savings`,
  `budget_in`, `budget_out`, `ignored`) — the common basis of budgets, cashflow and
  month totals (plan §7.3): it takes into account `in_budget`, category groups,
  loan payments, transfers to goal nodes, internal and debt transfers.

## Money: Amount

`Amount` is a newtype over `rust_decimal::Decimal`:
- always **serialized as a string** (precision, stable JSON and bincode);
- `Amount::parse("123.45")`, `Amount::from_f64`, `to_f64()` (for display/engine only),
- Decimal arithmetic (no floating-point errors), `round(n)`;
- in `ipc.gen.d.ts` money fields are declared as `type Amount = string;`.

## Price caches — the rate history

- `PricePoint { asset_key, value_usd, ts, source }` — one unit of an asset in USD.
- `FxPoint { currency, per_usd, ts, source }` — units of a currency per 1 USD.
- This is a **history**, not just the last point: per currency/asset one point an hour
  for the last week, older — one a day, at most 2000 points
  (`pricing/history.rs`).
- `refresh_all` brings the whole FX table; the history is kept only for currencies
  the vault uses (USD, the base, the budget currency, fiat in transactions, currencies
  of manual prices), the others keep the last point.
- The rate at a moment is the nearest point ≤ the moment, otherwise the first after. Fiat is worth
  `1 / per_usd`; a manual property price applies to the whole history.
- The A↔B cross rate goes through USD (`convert_amount`).

### Price stamp

When a transaction is saved, each side without a price gets `price_usd` at the rate
on the transaction date; what is entered by hand is not touched. `price_base` is not
recorded: the value in the base currency is computed as `price_usd × the base rate
on the transaction date`, so changing the base currency breaks nothing. No rate —
the transaction is saved with a "without a rate" mark and gets a price at the next
rate refresh (`restamp_missing`).

## Per-store analytics (DTOs, not in the vault)

### StoreValue — a portfolio slice by store (in `PortfolioSummary.by_store`)
`{ store_id, name, value_usd, value_base, pct }`

### StoreOverview — the full map of one wallet
The request `get_store_overview(store_id, include_children)`; `include_children`
affects PnL and transaction statistics, sums arrive both ways.

| Field | Type | Comment |
|---|---|---|
| `store` | `Store` | metadata |
| `ancestors` | `Vec<StoreCrumb { id, name, color }>` | from the root to the parent — breadcrumbs |
| `base_currency` | `String` | |
| `fx_per_usd` | `Amount` | the base/USD rate at the moment |
| `include_children` | `bool` | the area PnL and transactions were computed over |
| `own_usd`, `own_base` | `Amount` | funds on the node itself ("Main") |
| `subtree_usd`, `subtree_base` | `Amount` | the node + all sub-accounts |
| `realized_pnl_base` | `Amount` | realized PnL over the area |
| `positions` | `Vec<StorePosition>` | own positions |
| `subtree_positions` | `Vec<StorePosition>` | subtree positions, merged by asset |
| `children` | `Vec<StoreChildRow { store, value_usd, value_base, subtree_base, share }>` | direct sub-accounts; `share` — the share in the parent's `subtree`, 0..1 |
| `can_add_child` | `bool` | whether a sub-account can be created here (depth, archive) |
| `tx_count` | `u32` | the number of transactions in the area |
| `first_tx_at` | `String` | RFC3339 or empty |
| `last_tx_at` | `String` | RFC3339 or empty |

### StoreTreeNode — a tree node (`get_store_tree`)
`{ store, depth, path: Vec<String>, own_usd, own_base, subtree_usd, subtree_base, own_tx_count, can_add_child, children }`.
Roots and their children are sorted: live before archived, then by name. A node's
sum in the UI with children is `subtree_base`, for a leaf — `own_base`.

### StorePosition — one position inside a store
`{ asset: AssetRef, quantity: Amount, avg_cost?: Amount, price_usd: Amount, value_usd: Amount, value_base: Amount, unrealized_base: Amount }`

### RealizedPnl (global and per-store)
`{ total_base: Amount, legs: Vec<PnlLeg> }`, where `PnlLeg { asset, store_id, quantity, avg_cost, exit_price, pnl_base }`

### CachedPrice
`{ price_usd: Amount, ts: String }` — the last known rate of an asset to USD.

### Calculator DTO
- `CalcInput { mode: "converter"|"position", amount, price_in?, price_out?, target_price?, stop_price?, fee_pct? }`
- `CalcResult { gross_in, fee, spent, gross_out, net_out, exit_value, pnl, roi_pct, risk, risk_pct }` — all `Amount` (string).

## Exchange keys (VaultDoc.exchanges)

`ExchangeKeys` gained `spot_store_id?` and `futures_store_id?` — the sub-accounts
that confirmed spot and futures trades are written to; if they are not
set — a live sub-account with the binding `market(spot)` / `market(futures)`, otherwise the key's
wallet itself. The background polling interval is per exchange
(`ExchangeProfile.import_interval_sec`). `PendingImport` stores `is_maker`, `closed_size`,
`closed_pnl`; a futures trade with `closed_size > 0` is a position close.

The field `exchanges: Exchanges` was added to `VaultDoc` (schema version **1**, the format
was reset from v2 to v1 — backward compatibility is not preserved).

```rust
pub struct ExchangeKeys {
    pub store_id: String,  // the ID of the wallet (the exchange node) that owns the key
    pub api_key: String,
    pub api_secret: String,
    pub spot_store_id: Option<String>,    // where to write spot trades
    pub futures_store_id: Option<String>, // where to write futures
}

pub struct Exchanges {
    #[serde(default)]
    pub bybit: Vec<ExchangeKeys>,  // Bybit keys — one per exchange node
}
```

- Keys are stored **at the exchange node level** (a wallet with `kind = exchange`).
- Several accounts of one exchange = several exchange nodes (each with its own keys).
- Child accounts (with nesting) have no keys — go up the ancestors to the exchange node.
- Validation on save: the node must exist and have `kind == exchange`.

Security guarantees:

- Written only inside the encrypted container, like the rest of the document.
- `Debug` is implemented by hand and prints `api_key` as a mask and `api_secret` as
  `<redacted>` — a key cannot leak through logs or `assert_eq!` output.
- `Drop` zeroes all fields through `zeroize`.
- `#[ts(skip)]` — the type does not get into the generated TS types, it never
  leaves the backend.
- The client (`crates/core/src/exchange/bybit.rs`) contains **only** `GET`
  (`/v5/execution/list`). There are no code paths for placing, changing or canceling
  orders, so even a leaked key cannot move funds.

Versions: `VAULT_CONTAINER_VERSION = 1` (the container format did not change),
`VAULT_FORMAT_VERSION = 2` (the document schema).

## Versions and migrations

The payload is written in one of the formats, the `payload_fmt` byte of the header says which:

| Byte | Format | Who writes it |
|---|---|---|
| `0` | JSON | debug build |
| `1` | bincode (positional) | release builds before v2; **read-only** |
| `2` | MessagePack with field names | release builds since v2 |

bincode does not store field names, so any new field broke old
release files. JSON and MessagePack store names: a field added with
`#[serde(default)]` is read from an old v2 file without a version bump.

Reading: first `version`, then the document is parsed by the schema of that version and
migrated along the chain. The v1 schema is frozen in `crates/core/src/storage/v1.rs`
(types that changed in v2 are copied, the rest are reused) and is never edited.
v1 → v2 loses nothing: new fields get default values.
Before the first write after a migration the file is copied to `<vault>.v1.bak`;
an existing copy is not overwritten. A bincode file is rewritten as
MessagePack.

v2 → v3 (`storage/v2.rs`): the `automations` and `automation_runs` collections no longer
exist — each automation becomes a scheme with the same id and
`Workflow.automation` (the graph is built from the form), its `last_planned_at` becomes a
`schedule:t` schedule mark (there is no repeated catch-up), each run becomes a `WorkflowRun`
with the same outcome (`Created` → `Done` with a transaction, `Rejected` → `Rejected` with
the planned moment, `Skipped` → `Dropped { "skipped" }`). Old transactions
keep `TxOrigin::Automation` and are found by the same id. The rest of the v2 document
is read by the current types (`serde(flatten)`), the file copy is `<vault>.v2.bak`.
The bump is also needed for sync: a device with an old version does not sync
with v3 and does not run automations a second time. The "amount is approximate" mark is carried
by `TxOrigin::Workflow { planned_at, estimated }`, not by `TxMeta`: v1 bincode files are
read by field position, and a new field in `TxMeta` would break them.

A new incompatible change: freeze the current schema in `storage::vN`, bump
`VAULT_FORMAT_VERSION`, add a migration step and tests for both formats.

## Auto-reminders

`service/auto_reminders.rs`: `sync_auto_reminders(now, zone, ru)` creates,
moves and removes reminders with `source { kind, id }` from data — a loan and
credit card payment (`LiabilityTerms.due_day`), rent (`UtilityKind::Housing`),
utilities and meter readings (`PropertyInfo.utilities_pay_day`), a deposit's end,
a PDS contribution, points expiring, a debt with a contact, setting aside for a goal and its
deadline, property tax and personal income tax on deposits (only if there is a reason). The time is
`notifications.default_time`, in advance — the category's `lead_days`; a category with
`auto_reminders = false` creates none. The schedule is remembered in
`Reminder.source_key`: only a changed schedule sets the reminder again,
so sync does not cancel "done"; it does not
touch `user_edited` and `muted`. A month for which the payment (a transfer to a loan, a payment for a utilities item,
a reading) is already recorded closes by itself. Sync runs in the scheduler and
on `list_reminders`.
