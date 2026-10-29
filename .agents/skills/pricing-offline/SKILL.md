---
name: pricing-offline
description: >
  Prices, FX, providers and offline degradation in trade-app: PricingService,
  the fallback chain CoinGecko → CoinCap → Binance, open.er-api for currencies,
  the rate history inside the vault, the price stamp on a transaction and the
  rule "network is separate from the cache". Use when changing
  crates/core/src/pricing/**, commands/pricing.rs, model/vault.rs (price_cache,
  fx_cache), and for tasks like "price does not update" («цена не
  обновляется»), "offline" («офлайн»), "stale price", "currency rate" («курс
  валюты»), "new provider" («новый провайдер»), "cache" («кэш»), "refresh_all",
  "lags when typing a price" («тормозит при вводе цены»), "429", "rate limit".
  Triggers: pricing, price, prices, цена (price), котировки (quotes), FX, курс
  (rate), валюта (currency), USD, CoinGecko, CoinCap, Binance, er-api, TTL,
  cache, кэш (cache), offline, офлайн (offline), деградация (degradation),
  refresh_all, PricePoint, FxPoint, price_cache, fx_cache, search_assets,
  get_cached_price.
---

# Prices, FX and offline degradation

`crates/core/src/pricing/mod.rs` is the only module that goes to the network
for prices. All endpoints are free and **keyless**.

## Providers

| What | Chain |
|---|---|
| Crypto (spot) | CoinGecko (`/simple/price` by id) → CoinGecko (token by contract) → CoinCap → Binance ticker |
| ERC-20 and similar tokens | CoinGecko `/simple/token_price/{platform}` (needs `chain` + `contract`) |
| Currencies (FX) | `open.er-api.com/v6/latest/USD` → Frankfurt/ECB |

Constants at the top of the file: `COINGECKO_V3`, `COINCAP_V2`, `BINANCE_API`,
`ER_API`. The fallback chain lives in `price_usd(&self, asset)`: **four
sequential attempts, each on `if let Ok(...)`**. A failed attempt silently
moves on to the next; an error goes out only if none of them worked.

## Rate history inside the vault (plan §6.1)

- The cache lives **in the vault**: `doc.price_cache: Vec<PricePoint>` and
  `doc.fx_cache: Vec<FxPoint>`, i.e. it is encrypted and survives a restart.
  This is not an in-memory cache: restarting the application does not clear it.
- The cache is a **history**, not "the latest point": `pricing/history.rs`,
  `merge_prices` / `merge_fx`. Per series (currency or asset key) there is one
  point per hour (the fresh one wins), the last week (`HOURLY_KEEP`) hourly,
  older ones one point per day, no more than `HISTORY_CAP = 2000` points.
- `fetch_fx()` brings the **whole table** from er-api. History is kept only for
  the currencies from `UnlockedVault::fx_currencies_in_use()` (USD, the base
  currency, the budget currency, fiat in transactions and holdings, currencies
  of manual prices); the rest get only the single latest point, otherwise ~170
  currencies hourly would bloat the vault, which is re-encrypted on every
  write. Consequence: a currency you started using today has no past.
- Time is compared by unix seconds (`history::unix_secs`), not as a string:
  `…:00Z` and `…:00.5Z` sort the wrong way as strings.
- "At a moment" queries go through `RateBook` (`UnlockedVault::rate_book()`,
  `service/rates.rs`): the cache is parsed once, then a binary search.
  `fx_at` / `price_at` take the nearest point <= ts, otherwise the first one
  after; `history_price` for a chart looks at the past first (cache, transaction
  stamps) and only then at the future; `convert` is a cross rate via USD;
  `leg_price_in_base` is the price of a leg in the base currency at the rate of
  the transaction day.
- Fiat is valued through FX: `fiat:RUB` → `1 / per_usd(rub)`. A manual price of
  a property (D24) applies to the whole history. Derivatives have no price.

## Price stamp on save (plan §6.2)

`add_transaction` / `update_transaction` call `stamp_prices`: every side
without `price_usd` and `price_base` gets `price_usd = price_at(asset,
timestamp)`. Anything entered by hand is not overwritten. The stamp does **not**
write `price_base`: the base currency changes, and the rate to it is taken from
the FX history at the transaction date. No rate means the transaction is saved
without a price; after applying rates `refresh_all` calls `restamp_missing()`
and writes the count into `RefreshResult.restamped`.

## Iron rule: the network is separate from document mutation

This is architectural rule #4 of the project, and it is not about style but
about borrowing:

```rust
// NETWORK: pure, do not touch doc, safe for .await
async fn fetch_prices(&self, assets: &[AssetRef]) -> Result<Vec<PricePoint>>
async fn fetch_fx(&self)                            -> Result<Vec<FxPoint>>

// MUTATION: apply what was already fetched (do not persist: refresh writes once)
fn UnlockedVault::apply_prices(&mut self, fresh: &[PricePoint])
fn UnlockedVault::apply_fx(&mut self, fresh: &[FxPoint])   // knows whose history to keep
fn UnlockedVault::restamp_missing(&mut self) -> usize
```

**Why:** `doc` lives inside `Mutex<Option<UnlockedVault>>`, and the guard must
not be held across `.await`. A pure `fetch_*` lets you take the needed data
from the vault, release the guard, await the network in peace, take the guard
again and apply. The reference is `refresh_all` in
`crates/app/src/commands/pricing.rs`:

```rust
let assets = { let g = require_vault(&state)?; /* …copy… */ };
let prices = state.pricing.fetch_prices(&assets).await?;          // await without guard
let fx     = state.pricing.fetch_fx().await?;
let mut g = require_vault(&state)?;                                // guard again
let vault = g.as_mut().unwrap();
vault.apply_prices(&prices);
vault.apply_fx(&fx);
let restamped = vault.restamp_missing();
vault.persist()?;
```

Never write `&mut doc` and `.await` in the same function. If it seems you
"need to check the cache first, then load the rest", do that **outside**, by
copying the `Vec<AssetRef>`/the base currency name.

## Offline: what happens without a network

Individual commands **do not fail** on a network error: they return an empty
result with a notice, leaving the state as it is:

| Command | Behavior on failure |
|---|---|
| `get_cached_price` / `convert_amount` | cache-only, never touch the network, so it is safe to call on every input; `at` is the moment for which the rate is needed |
| `search_assets` / `inspect_tx` / `bybit_executions` | an empty list + `notice` (`empty`/`unavailable`/`choose_network`) |
| `refresh_all` | the error goes out (`errors.NETWORK`/`errors.PRICING`), the cache is **left untouched** |

The deal form is not reset in the meantime: what the user typed by hand stays
in place. This is a deliberate requirement, not a bug: "the network is separate
from the cache" must not mean "lose the input".

## Anti-patterns

- **Do not make a price request in an `input` handler / on every keystroke.**
  That guarantees a rate limit. To show a price hint there is
  `get_cached_price` (no network). Real requests go on the refresh button or on
  a debounced timer.
- **Do not fire one HTTP request per asset.** `fetch_prices` calls
  `price_usd(asset).await` **sequentially for every asset**, and inside
  `price_usd` there are up to four attempts. With 50 assets that is up to 200
  requests. `refresh_all` deduplicates assets by `asset.key()` and filters out
  fiat, but batching is still the right way.
- **`fetch_prices` propagates the error of the first failed asset** (`?` inside
  the loop), so there is no partial success: the whole batch fails. If you need
  "count what we managed to get", that is a separate task: change it
  explicitly and cover it with a test.
- **Do not write prices and rates as floating-point numbers.** A price is
  `Amount` (Decimal); it arrives in TS as a string. `f64` is allowed only in
  `market/` (chart candles, see the `vault-crypto` skill).
- **Do not hardcode `source`.** It goes into `PricePoint`/`FxPoint` and is
  visible in the vault data: `"cryptoprice"` / `"open.er-api"` are the existing
  values.
- **Do not add an API key.** The whole scheme is built on keyless endpoints; a
  secret key would require a new secrets branch in the vault (see
  `vault-crypto`).

## Event

After success `refresh_all` emits `prices-updated` with a `RefreshResult`. The
frontend subscribes through `onEvent<T>` from `src/shared/api/ipc.ts` and
invalidates the query keys (`['summary']`, `['store', id]`,
`['store-history', id]`); when `restamped > 0` `AppShell` also invalidates the
transaction lists, and the "no price" badge (UI string `tx.noPrice`) goes away.
Register new events here as well and in `docs/ipc.md`.

## Tests and checks

The history unit tests are in `pricing/history.rs`, the stamp and rates at a
date in `service/rates.rs`; all of them run without a network. Everything that
goes over HTTP is checked **only manually**: `mise run dev`, then `refresh_all`
in the UI.

```bash
mise run test        # do not break the pure cache logic
mise run typecheck   # if you changed a DTO
mise run check       # before committing
```

If you changed `price_cache`/`fx_cache` in `model/vault.rs`, that is the
**vault schema**: do **not** bump the version, the field is additive via
`#[serde(default)]` (skill `vault-crypto`, AGENTS.md rule 5). If the type goes
through IPC, run `mise run gen-types`.
