# Модель данных

Все сущности живут в **одном шифрованном файле** (vault). «Хранилище (store)» —
это контейнер портфеля, а не актив; актив представлен `Holding` внутри store.

## VaultDoc (корень документа)

| Поле | Тип | Комментарий |
|---|---|---|
| `version` | `u32` | `VAULT_FORMAT_VERSION`, зафиксирован на `1`. **Не бампать без явного разрешения пользователя**: `decode` отвергает любую другую версию, а миграции нет. Аддитивные поля — через `#[serde(default)]` |
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

`Store { id, name, kind, color, archived, note, parent_id? }`, где `kind` — один из:
`exchange`, `hot_wallet`, `cold_wallet`, `bank`, `cash`. Хранилище можно архивировать;
архивные не считаются в сводке.

### Дерево: кошелёк → субсчёт → субсчёт

`parent_id = None` — кошелёк (корень), иначе субсчёт. Глубина ограничена
`MAX_STORE_DEPTH = 2` (корень — глубина 0), то есть **три уровня**. У узла с
субсчетами есть и свои средства и операции («Основной» в UI).

Правила проверяются при записи (`validate_placement` в `service.rs`); чтение
терпимо: vault, отредактированный руками, всё равно открывается.

| # | Правило | Код ошибки |
|---|---|---|
| R1 | Родитель существует | `MISSING` |
| R2 | Узел не вкладывается в себя и в своё поддерево | `STORE_CYCLE` |
| R3 | `depth(родитель) + 1 + высота(поддерева узла) ≤ 2` | `STORE_TOO_DEEP` |
| R4 | Родитель не архивирован | `STORE_PARENT_ARCHIVED` |
| R5 | Имя уникально среди соседей (без учёта регистра и пробелов по краям); корни — среди корней | `STORE_NAME_TAKEN` |
| R6 | Корень не становится субсчётом (`move_store`) | `STORE_ROOT_MOVE` |
| R8 | Архивация — каскадом вниз; восстановление — тоже, но не под архивным родителем | `STORE_RESTORE_PARENT_FIRST` |
| R9 | Удаление — только без операций и без субсчетов | `INVALID_INPUT` |

Тип и цвет субсчёта по умолчанию берутся от родителя — это решает UI, core тип не
навязывает. Место узла меняет только `move_store`; `update_store` правит имя,
цвет и заметку.

### StoreScope — область отчёта

`StoreScope { store_ids: Vec<String>, include_children: bool }` — единый фильтр
по кошелькам: пустой список = весь vault, `include_children` (по умолчанию
`true`) раскрывает каждый id в поддерево. На нём построены список операций,
история стоимости и realized PnL узла. Пока не хранится в vault.

## AssetRef

`{ symbol, name, coingecko_id?, chain?, contract?, quote }`.
`chain` — вариант `Chain` (`bitcoin`, `ethereum`, `bsc`, `solana`, …, `fiat`).
Для фиата строится `AssetRef::fiat(symbol, name)` с `chain: Fiat`.
`AssetRef::key()` — канонический ключ для кэшей и сравнений
(используется в `PricePoint.asset_key`).

## Transaction (источник истины)

`Transaction` — **закрытый enum**, а не плоская структура. Каждый вариант несёт
только свои поля, поэтому невозможная запись не представима: у `BankTransferTx`
нет DEX-`pool_id`, у `CashFlowTx` нет IBAN.

```rust
enum Transaction {
    Income(IncomeTx), Expense(ExpenseTx), Payment(PaymentTx), Fee(FeeTx),
    BankTransfer(BankTransferTx), CardPayment(CardPaymentTx),
    WalletTransfer(WalletTransferTx), DefiAction(DefiActionTx),
    ExchangeTrade(ExchangeTradeTx), Transfer(TransferTx), CashFlow(CashFlowTx),
}
```

Общая часть вынесена отдельно, чтобы не расходилась между вариантами:

| Структура | Поля |
|---|---|
| `TxMeta` | `id`, `timestamp` (RFC3339), `comment`, `categories` |
| `PriceStamp` | `price_usd?`, `price_base?`, `fx_override?` |
| `Flow` | `In` \| `Out` — знак задаётся данными, а не соглашением |
| `BankTransferTx` | `store_id`, `asset`, `amount`, `flow`, `rail`, `counterparty_name`, `counterparty_account?`, `reference?`, `fee?`, `fee_asset?`, `meta`, `price` |
| `ExchangeTradeTx` | `store_id`, `sold { asset, qty }`, `bought { asset, qty }`, `fee?`, `fee_asset?`, `order_id?`, `market?`, `liquidity?`, `meta`, `price` |
| `TransferTx` | `from_store_id`, `to_store_id`, `asset`, `amount`, `fee?`, `fee_asset?`, `network?`, `tx_hash?`, `meta`, `price` |

Остальные варианты — по тому же принципу; полный список полей даёт
`mise run gen-types` → `src/shared/api/ipc.gen.d.ts`.

### Ноги (Leg)

`Transaction::legs()` разворачивает запись в список `Leg` — по одному на
`(store, asset)`. Трейд и перевод дают по две ноги, остальные — одну. Ноги
**не хранятся**: они вычисляются при чтении, а всё, что суммирует деньги
(`recompute_holdings`, `history_series`, `realized_pnl`), работает только с
ними и не различает виды операций.

Знак берётся из `Flow::signed()` — единственного места, где направление
становится числом.

### Привязка видов к типу кошелька

`Transaction::tags_for_store_kind(StoreKind)` — что кошелёк данного типа
принимает. Возвращается двумя группами: `COMMON_TAGS` (работают везде, включая
`transfer` — он доступен от любого кошелька, потому что второй конец может быть
любым) и специфичные для кошелька.

| Тип кошелька | Специфичные виды | По умолчанию |
|---|---|---|
| `exchange` | `exchange_trade`, `wallet_transfer` | `exchange_trade` |
| `hot_wallet` / `cold_wallet` | `wallet_transfer`, `defi_action` | `wallet_transfer` |
| `bank` | `bank_transfer`, `card_payment` | `bank_transfer` |
| `cash` | `cash_flow` | `cash_flow` |

Фронт спрашивает это командой `transaction_kinds_for_store_kind` и своей копии
не держит. `match` в `specific_tags_for_store_kind` без wildcard-руки, поэтому
новый `StoreKind` не соберётся, пока для него не описан набор.

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
Запрос `get_store_overview(store_id, include_children)`; `include_children`
влияет на PnL и статистику операций, суммы приходят обоими способами.

| Поле | Тип | Комментарий |
|---|---|---|
| `store` | `Store` | метаданные |
| `ancestors` | `Vec<StoreCrumb { id, name, color }>` | от корня до родителя — хлебные крошки |
| `base_currency` | `String` | |
| `fx_per_usd` | `Amount` | курс base/USD на момент |
| `include_children` | `bool` | какой областью посчитаны PnL и операции |
| `own_usd`, `own_base` | `Amount` | средства на самом узле («Основной») |
| `subtree_usd`, `subtree_base` | `Amount` | узел + все субсчета |
| `realized_pnl_base` | `Amount` | реализованный PnL по области |
| `positions` | `Vec<StorePosition>` | свои позиции |
| `subtree_positions` | `Vec<StorePosition>` | позиции поддерева, сведённые по активу |
| `children` | `Vec<StoreChildRow { store, value_usd, value_base, subtree_base, share }>` | прямые субсчета; `share` — доля в `subtree` родителя, 0..1 |
| `can_add_child` | `bool` | можно ли создать субсчёт здесь (глубина, архив) |
| `tx_count` | `u32` | кол-во операций в области |
| `first_tx_at` | `String` | RFC3339 или пусто |
| `last_tx_at` | `String` | RFC3339 или пусто |

### StoreTreeNode — узел дерева (`get_store_tree`)
`{ store, depth, path: Vec<String>, own_usd, own_base, subtree_usd, subtree_base, own_tx_count, can_add_child, children }`.
Корни и их дети отсортированы: живые раньше архивных, дальше по имени. Сумма
узла с детьми в UI — `subtree_base`, листа — `own_base`.

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
