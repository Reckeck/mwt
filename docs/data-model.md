# Модель данных

Все сущности живут в **одном шифрованном файле** (vault). «Хранилище (store)» —
это контейнер портфеля, а не актив; актив представлен `Holding` внутри store.

## VaultDoc (корень документа)

| Поле | Тип | Комментарий |
|---|---|---|
| `version` | `u32` | `VAULT_FORMAT_VERSION`; бамп на ломающее изменение схемы + миграция |
| `id` | `String` | uuid |
| `created_at` | `String` | RFC3339 UTC |
| `settings` | `Settings` | vault-scoped: decimals, виджеты, интервалы, трей |
| `stores` | `Vec<Store>` | контейнеры портфеля |
| `holdings` | `Vec<Holding>` | derived from transactions (`recompute_holdings`) |
| `transactions` | `Vec<Transaction>` | история операций (источник истины) |
| `categories` | `Vec<String>` | пользовательские категории (после системных: salary/rewards/trading/staking/defi/other) |
| `price_cache` | `Vec<PricePoint>` | курс актива → USD, с TTL |
| `fx_cache` | `Vec<FxPoint>` | units per 1 USD для валют |

### Settings — только vault-scoped

`display_decimals` (по умолчанию 8), `dashboard_widgets`,
`fx_refresh_interval_sec` (300), `minimize_to_tray` (true),
`bybit_import_interval_sec` (900; `0` — выкл).

Настройки, описывающие **пользователя**, а не этот vault, живут не в документе,
а рядом с приложением — см. «Настройки приложения» ниже.

## Настройки приложения (вне vault)

Тема, язык, базовая валюта и избранные валюты следуют за пользователем, а не за
vault: они переживают открытие другого хранилища и читаются **до** разблокировки
(поэтому экран входа сразу рендерится на нужном языке). Поэтому они не
зашифрованы и не дублируются в каждом vault — лежат обычным JSON в
`app_config_dir`.

`crates/core/src/appconfig.rs`, два файла:

| Файл | Что |
|---|---|
| `app_settings.json` | `AppSettings { theme, locale, base_currency, favorite_currencies[] }` |
| `currencies.json` | кэш каталога валют, `SupportedCurrencies { fiat[], crypto[] }` |

Дефолты: `theme: auto`, `locale: auto`, `base_currency: "usd"`, favorites пуст.
Битый или отсутствующий файл даёт дефолты — экран входа обязан отрисоваться
всегда.

**Нормализация при записи:** `base_currency` → trim + lowercase, пустое → `usd`;
коды избранных валют → uppercase, только 3 буквы, дедупликация.

### Каталог валют: лестница деградации

`get_supported_currencies` никогда не падает — выбирается первая доступная ступень:

1. **сеть** — `open.er-api.com` (фиат) + CoinGecko `/coins/list` (крипта);
   успешный ответ кэшируется в `currencies.json`;
2. **кэш** — последний успешный ответ, если сеть недоступна (упавшая половина
   берётся из кэша, а не выбрасывается);
3. **встроенный список** — `FALLBACK_FIAT` / `FALLBACK_CRYPTO`, если промаха
   ещё не было ни разу.

`notice` в ответе говорит, какая ступень использована: `builtin`, `cached` или
`null` (ответили только что). UI показывает по нему предупреждение, что список
может быть неполным.

### Base currency — поле времени выполнения, не документа

`UnlockedVault::base_currency` живёт в памяти, а не в `doc`: базовая валюта
настраивается приложением, поэтому хранилище не может о ней знать. После
`open`/`create` слой IPC вызывает `sync_base_currency`, который заталкивает
значение в vault; смена базовой валюты пересчитывает холдинги и средние
стоимости (`set_base_currency` → `recompute_holdings`), чтобы цифры на экране
соответствовали подписи. Уже записанные в транзакциях отпечатки `price_base` не
переписываются — они остаются такими, какими были зафиксированы в момент сделки.

## Store / StoreKind

`Store { id, name, kind, color, archived, note }`, где `kind` — один из:
`exchange`, `hot_wallet`, `cold_wallet`, `bank`, `cash`. Хранилище можно архивировать;
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
| `id`, `group_id`? | `String` | group_id общий для связанных ног (transfer/trade) |
| `kind` | `TxKind` | `income`, `expense`, `payment`, `transfer`, `trade`, `fee` |
| `store_id` | `String` | главное хранилище (откуда расход / куда доход) |
| `asset` | `AssetRef` | |
| `amount` | `Amount` | знак учитывается при агрегации (`signed_delta`); у income положительный, у expense/payment — знак траты |
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
| `categories` | `Vec<String>` | теги пользователя, **сколько угодно на одну операцию** |

Знаковая конвенция: `amount > 0` — актив входит в `store_id` (кроме `expense`
и `payment`, где положительное = потрачено); `amount < 0` — выходит из
`store_id`. Ноги перевода: исходящая нога имеет отрицательный `amount` на
`store_id` + заполненный `to_store_id`; входящая — положительный с
`store_id == to_store_id` исходящей.

### Виды операций

`payment` — оплата товара или услуги (магазин, подписка, счёт). Отдельный вид
от `expense`, потому что это расход, за который пользователь что-то
*получил*: именно по его тегам (`categories`) потом строится аналитика трат.
Деньги уходят так же, как у `expense` (`signed_delta` → `-amount`), поэтому
payment уменьшает holdings, но, как и любой выход, не трогает avg_cost.

### Категории

- Словарь — `VaultDoc.categories` (в нём же живут системные `salary`,
  `rewards`, `trading`, `staking`, `defi`, `other`).
- Одна операция несёт **любое количество** тегов: `food` + `weekend` +
  `home`. Это осознанно — по тегам строятся будущие графики трат, и одна
  покупка часто относится сразу к нескольким срезам.
- `UnlockedVault::adopt_categories()` нормализует список перед сохранением
  (trim → lowercase → пустые отброшены → дубликаты сняты с сохранением
  порядка) и добавляет новые теги в словарь, чтобы пикер всегда предлагал
  всё, что реально используется.
- Импорт с биржи проставляет единственный тег `trading`.

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

## Ключи бирж (VaultDoc.exchanges)

В `VaultDoc` добавлено поле `exchanges: Exchanges` (схема версии **1**, формат
сброшен с v2 на v1 — обратная совместимость не сохраняется).

```rust
pub struct ExchangeKeys {
    pub store_id: String,  // ID кошелька (узла-биржи), которому принадлежит ключ
    pub api_key: String,
    pub api_secret: String,
}

pub struct Exchanges {
    #[serde(default)]
    pub bybit: Vec<ExchangeKeys>,  // ключи Bybit — по одному на узел-биржу
}
```

- Ключи хранятся **на уровне узла-биржи** (кошелька с `kind = exchange`).
- Множество аккаунтов одной биржи = несколько узлов-бирж (каждый со своими ключами).
- Дочерние счета (при вложенности) ключей не имеют — подъём по предкам к узлу-биржи.
- Валидация при сохранении: узел должен существовать и иметь `kind == exchange`.

Гарантии безопасности:

- Пишутся только внутрь зашифрованного контейнера, как и остальной документ.
- `Debug` реализован вручную и печатает `api_key` как маску, а `api_secret` как
  `<redacted>` — ключ не может утечь через логи или `assert_eq!`-вывод.
- `Drop` зануляет все поля через `zeroize`.
- `#[ts(skip)]` — тип не попадает в сгенерированные TS-типы, наружу он не
  выходит никогда.
- Клиент (`crates/core/src/exchange/bybit.rs`) содержит **только** `GET`
  (`/v5/execution/list`). Кодовых путей для постановки, изменения или отмены
  ордеров нет, поэтому даже утёкший ключ не позволит двигать средства.

Версии: `VAULT_CONTAINER_VERSION = 1` (формат контейнера не менялся),
`VAULT_FORMAT_VERSION = 1` (схема документа — сброс с v2 на v1, миграций нет).
