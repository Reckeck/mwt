# Модель данных

Все сущности живут в **одном шифрованном файле** (vault). «Хранилище (store)» —
это контейнер портфеля, а не актив; актив представлен `Holding` внутри store.

## VaultDoc (корень документа)

| Поле | Тип | Комментарий |
|---|---|---|
| `version` | `u32` | `VAULT_FORMAT_VERSION`; бамп на ломающее изменение схемы + миграция |
| `id` | `String` | uuid |
| `created_at` | `String` | RFC3339 UTC |
| `settings` | `Settings` | базовая валюта, тема, локаль, decimals |
| `stores` | `Vec<Store>` | контейнеры портфеля |
| `holdings` | `Vec<Holding>` | derived from transactions (`recompute_holdings`) |
| `transactions` | `Vec<Transaction>` | история операций (источник истины) |
| `categories` | `Vec<String>` | пользовательские категории (после системных: salary/rewards/trading/staking/defi/other) |
| `price_cache` | `Vec<PricePoint>` | курс актива → USD, с TTL |
| `fx_cache` | `Vec<FxPoint>` | units per 1 USD для валют |
| `sim` | `SimState` | стратегии + прогоны симулятора |

### Settings
`base_currency` (`usd` по умолчанию), `theme` (`auto|light|dark`),
`locale` (`auto|en|ru`), `display_decimals` (по умолчанию 8).

## Store / StoreKind

`Store { id, name, kind, color, archived, note }`, где `kind` — один из:
`exchange`, `hot_wallet`, `cold_wallet`, `bank`, `cash`, `paper`
(последний — виртуальные деньги симулятора). Хранилище можно архивировать;
архивные не считаются в сводке.

## AssetRef

`{ symbol, name, coingecko_id?, chain?, contract?, quote }`.
`chain` — вариант `Chain` (`bitcoin`, `ethereum`, `bsc`, `solana`, …, `fiat`).
Для фиата строится `AssetRef::fiat(symbol, name)` с `chain: Fiat`.
`AssetRef::key()` — канонический ключ для кэшей и сравнений
(используется в `PricePoint.asset_key`).

## Transaction (источник истины)

| Поле | Тип | Комментарий |
|---|---|---|
| `id`, `group_id`? | `String` | group_id общий для связанных ног (transfer/trade/paper-run) |
| `kind` | `TxKind` | `income`, `expense`, `transfer`, `trade`, `fee`, `paper_trade` |
| `store_id` | `String` | главное хранилище (откуда расход / куда доход) |
| `asset` | `AssetRef` | |
| `amount` | `Amount` | знак учитывается при агрегации (`signed_delta`); у income положительный, у expense — знак траты |
| `to_store_id`? | `String` | сторона-контрагент для transfer/trade |
| `price_usd`? | `Amount` | за единицу на момент сделки |
| `price_base`? | `Amount` | цена за единицу в базовой валюте |
| `fx_override`? | `Amount` | ручной курс (base за 1 usd) на момент сделки |
| `fee`? | `Amount` | в единицах `fee_asset` (или актива сделки) |
| `fee_asset`? | `String` | символ актива комиссии |
| `timestamp` | `String` | RFC3339; события-границы для history_series |
| `network`? | `Chain` | сеть для on-chain операций |
| `tx_hash`? | `String` | explorer-ссылка строится из хеша и сети |
| `comment` | `String` | свободный текст (обязательное поле у каждой сделки) |
| `category`? | `String` | из `categories` |

Знаковая конвенция: `amount > 0` — актив входит в `store_id` (кроме `expense`, где положительное = потрачено); `amount < 0` — выходит из `store_id`. Ноги перевода: исходящая нога имеет отрицательный `amount` на `store_id` + заполненный `to_store_id`; входящая — положительный с `store_id == to_store_id` исходящей.

### Правила агрегации (holding)
- Только сущности с ненулевым количеством и не-архивные хранилища.
- avg_cost обновляется взвешенно на buy/income.
- Пересчёт — `UnlockedVault::recompute_holdings()` после каждой мутации.

## Деньги: Amount

`Amount` — newtype над `rust_decimal::Decimal`:
- всегда **сериализуется строкой** (точность, стабильный JSON и bincode);
- `Amount::parse("123.45")`, `Amount::from_f64`, `to_f64()` (только для отображения/engine),
- арифметика Decimal (без плавающих ошибок), `round(n)`;
- в `ipc.gen.d.ts` денежные поля объявлены как `type Amount = string;`.

## Кэши цен

- `PricePoint { asset_key, value_usd, ts, source }` — один юнит актива в USD.
- `FxPoint { currency, per_usd, ts, source }` — единиц валюты за 1 USD.
- Обновление: `PricingService::fetch_prices/fetch_fx` (сеть) +
  `apply_prices/apply_fx` (кэш в `VaultDoc`). TTL: `PRICE_TTL_SECONDS`,
  `FX_TTL_SECONDS` (см. `pricing/mod.rs`).
- Вне сети используются последние известные курсы; fallback в истории —
  последний `price_usd` транзакции по активу.

## Per-store аналитика (DTO, не в vault)

### StoreValue — срез портфеля по хранилищу (в `PortfolioSummary.by_store`)
`{ store_id, name, value_usd, value_base, pct }`

### StoreOverview — полная карта одного кошелька
| Поле | Тип | Комментарий |
|---|---|---|
| `store` | `Store` | метаданные |
| `base_currency` | `String` | |
| `fx_per_usd` | `Amount` | курс base/USD на момент |
| `total_usd` | `Amount` | сумма позиций в USD |
| `total_base` | `Amount` | сумма позиций в базовой валюте |
| `realized_pnl_base` | `Amount` | реализованный PnL только по этому store |
| `positions` | `Vec<StorePosition>` | открытые позиции |
| `tx_count` | `u32` | кол-во операций (ноги transfer/trade считаются по каждому store) |
| `first_tx_at` | `String` | RFC3339 или пусто |
| `last_tx_at` | `String` | RFC3339 или пусто |

### StorePosition — одна позиция внутри store
`{ asset: AssetRef, quantity: Amount, avg_cost?: Amount, price_usd: Amount, value_usd: Amount, value_base: Amount, unrealized_base: Amount }`

### RealizedPnl (global и per-store)
`{ total_base: Amount, legs: Vec<PnlLeg> }`, где `PnlLeg { asset, store_id, quantity, avg_cost, exit_price, pnl_base }`

### CachedPrice
`{ price_usd: Amount, ts: String }` — последний известный курс актива к USD.

### Calculator DTO
- `CalcInput { mode: "converter"|"position", amount, price_in?, price_out?, target_price?, stop_price?, fee_pct? }`
- `CalcResult { gross_in, fee, spent, gross_out, net_out, exit_value, pnl, roi_pct, risk, risk_pct }` — все `Amount` (string).
## Ключи бирж (VaultDoc.bybit)

Опциональное поле `bybit: Option<ExchangeKeys>` в `VaultDoc`. Появилось в
схеме версии **2**; в `VaultDoc` идёт последним полем и помечено
`#[serde(default)]`, поэтому vault версии 1 открывается без миграции данных —
просто `bybit = None`.

```rust
pub struct ExchangeKeys {
    pub api_key: String,
    pub api_secret: String,
}
```

Гарантии безопасности:

- Пишется только внутрь зашифрованного контейнера, как и остальной документ.
- `Debug` реализован вручную и печатает `api_key` как маску, а `api_secret` как
  `<redacted>` — ключ не может утечь через логи или `assert_eq!`-вывод.
- `Drop` зануляет оба поля через `zeroize`.
- `#[ts(skip)]` — тип не попадает в сгенерированные TS-типы, наружу он не
  выходит никогда.
- Клиент (`crates/core/src/exchange/bybit.rs`) содержит **только** `GET`
  (`/v5/execution/list`). Кодовых путей для постановки, изменения или отмены
  ордеров нет, поэтому даже утёкший ключ не позволит двигать средства.

Версии: `VAULT_CONTAINER_VERSION = 1` (формат контейнера не менялся),
`VAULT_FORMAT_VERSION = 2` (схема документа). Миграция v1→v2 — в
`crates/core/src/storage/mod.rs`, покрыта тестами для JSON и bincode.
