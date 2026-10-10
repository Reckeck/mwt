# Модель данных

Все сущности живут в **одном шифрованном файле** (vault). «Хранилище (store)» —
это контейнер портфеля, а не актив; актив представлен `Holding` внутри store.

## VaultDoc (корень документа)

| Поле | Тип | Комментарий |
|---|---|---|
| `version` | `u32` | `VAULT_FORMAT_VERSION`, зафиксирован на `1`. **Не бампать без явного разрешения пользователя**: `decode` отвергает любую другую версию, а миграции нет. Release пишет bincode, поэтому любое новое поле ломает старые release-файлы — схема заморожена целиком (план D11). Файл версии 1, не подходящий под схему, даёт `INCOMPATIBLE_VAULT` (конвертера нет) |
| `id` | `String` | uuid |
| `created_at` | `String` | RFC3339 UTC |
| `settings` | `Settings` | vault-scoped: decimals, виджеты, интервалы, трей |
| `stores` | `Vec<Store>` | контейнеры портфеля |
| `holdings` | `Vec<Holding>` | derived from transactions (`recompute_holdings`) |
| `transactions` | `Vec<Transaction>` | история операций (источник истины) |
| `categories` | `Vec<Category>` | категории с группами; новый vault получает набор по умолчанию (`default_categories()`) |
| `price_cache` | `Vec<PricePoint>` | курс актива → USD, с TTL |
| `fx_cache` | `Vec<FxPoint>` | units per 1 USD для валют |
| `exchanges` | `Exchanges` | ключи бирж (см. ниже); в TS не экспортируется |
| `pending_imports` | `Vec<PendingImport>` | сделки Bybit, ждущие подтверждения |
| `contacts` | `Vec<Contact>` | внешние стороны и их реквизиты |
| `categorization_rules` | `Vec<CategorizationRule>` | «текст → категория» |
| `manual_prices` | `Vec<ManualPrice>` | ручная оценка имущества |
| `automations`, `automation_runs`, `budgets`, `goals`, `reminders`, `price_alerts`, `utility_items`, `meter_readings`, `enrichments` | `Vec<…>` | типы фаз 3–7, определены заранее ради одного разлома схемы; пока пустые |

### Settings — только vault-scoped

`display_decimals` (по умолчанию 8), `dashboard_widgets`,
`fx_refresh_interval_sec` (300), `minimize_to_tray` (true),
`bybit_import_interval_sec` (900; `0` — выкл), `savings_rate` (доля дохода
на накопления, 0..1, по умолчанию `0.10`), `daily_budget_mode` (`month` —
месяц с переносом остатка, по умолчанию; `until_income` — до следующего
дохода), `budget_currency` (пусто = базовая валюта).

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

`Store { id, name, kind, color, archived, note, parent_id?, contact_id?, liability?, in_budget }`,
где `kind` — один из: `exchange`, `hot_wallet`, `cold_wallet`, `bank`, `cash`,
`asset` (имущество с ручной оценкой: квартира, машина), `liability` (кредит,
ипотека, кредитка — баланс отрицательный). Хранилище можно архивировать.

- `in_budget` — траты и доходы узла попадают в бюджет. По умолчанию у корня —
  `cash`/`bank` да, остальные нет; субсчёт берёт значение родителя (D22).
- `liability: LiabilityTerms { kind, rate?, limit?, due_day?, monthly_payment? }` —
  только у `liability`; `limit` ограничивает кредитку для автоматики.
- `contact_id` — чей это счёт (банк, обменник); только у корня (R10).

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

`Transaction` — **закрытый enum** с внешним тегом (bincode не умеет внутренние
теги). Четыре глагола и четыре специальных вида:

```rust
enum Transaction {
    Topup(TopupTx),       // Пополнить
    Payment(PaymentTx),   // Оплатить
    Transfer(TransferTx), // Перевести (один актив)
    Convert(ConvertTx),   // Обменять (разные активы)
    TradeSpot(TradeSpotTx), TradeFutures(TradeFuturesTx),
    Defi(DefiTx), AssetSell(AssetSellTx),
}
```

| Структура | Поля |
|---|---|
| `TxMeta` | `id`, `timestamp` (RFC3339 UTC), `comment`, `categories` (ключи), `flow_id?` (цепочка), `contact_id?` («с кем / через кого»), `origin` (`manual` \| `import { exec_id }` \| `automation { automation_id, planned_at }`) |
| `PriceStamp` | `price_usd?`, `price_base?`, `fx_override?` — у каждой оценённой стороны |
| `Fee` | `{ asset, amount, price }` — вместо пары `fee` + `fee_asset` |
| `AccountRef` | `store(id)` \| `contact { contact_id, account_id? }` \| `raw(text)` — одна сторона движения |
| `TopupTx` / `PaymentTx` | `store_id`, `asset`, `amount`, `fee?`, `from?` / `to?` (не `store`), `details: KindDetails`, `price` |
| `TransferTx` | `from`, `to` (≥ 1 сторона `store`), `asset`, `amount` (получатель получает ровно его), `fee?` (платит отправитель), `network?`, `tx_hash?`, `debt?: DebtEffect`, `price` |
| `ConvertTx` | `from`/`to: ConvertSide { account, asset, amount, price }`, `fee?`, `rate_override?` |
| `TradeSpotTx` | `store_id`, `sold`/`bought: TradeSide { asset, qty, price }`, `fee?`, `liquidity?`, `order_id?` |
| `TradeFuturesTx` | `store_id`, `contract` (дериватив), `settle`, `direction` (long/short), `action` (open/close), `quantity`, `fill_price`, `leverage?`, `fee?`, `closed_pnl?` (только close), `order_id?`, `category` (linear/inverse), `price` (цена settle) |
| `DefiTx` | `store_id`, `action` (stake/unstake/claim/add_liquidity/remove_liquidity), `asset`, `amount`, `protocol?`, `pool_id?`, `network?`, `tx_hash?`, `fee?`, `price` |
| `AssetSellTx` | `store_id`, `asset`, `unit_price`, `quantity`, `currency`, `proceeds_to?` (узел), `fee?`, `price` |
| `KindDetails` | `cash_breakdown?` (только `cash`, Σ = сумма), `unit_price?` (только `asset`: цена единицы в базовой валюте на дату покупки), `network?`, `tx_hash?` (только крипто-кошельки) |

### Ноги (Leg)

`Transaction::legs()` разворачивает запись в `Leg { store_id, asset, amount (со
знаком), role, to_store_id?, price_* }`. Ноги **не хранятся**; всё, что суммирует
деньги, работает только с ними. Ноги есть только у сторон-`store`: перевод
контакту даёт одну ногу.

`role`: `principal` — само движение; `fee` — комиссия, отдельная исходящая нога
(раньше комиссии записывались и игнорировались, холдинги были завышены);
`pnl` — реализованный PnL закрытия фьючерса в валюте расчёта.

Деривативы — `AssetRef::perp(биржа, символ)`, ключ `perp:{биржа}:{символ}`. Позиция
лежит как holding контракта, но **не входит** в стоимость, капитал и историю;
на странице кошелька — блок «Открытые позиции».

Realized PnL: продажей считается только исходящая `principal`-нога с известной
ценой и без второго своего узла; `fee` идёт отдельной суммой (`fees_base`),
`pnl` — напрямую.

### Привязка видов к типу кошелька

`allowed_kinds(StoreKind)` и `default_kind(StoreKind)` в `model/transaction.rs` —
единственный источник; фронт спрашивает `transaction_kinds_for_store_kind`.

| Тип | Глаголы | Спец. | По умолчанию |
|---|---|---|---|
| `cash`, `bank` | все четыре | — | Оплатить |
| `exchange` | все четыре | спот, фьючерсы | спот |
| `hot_wallet`, `cold_wallet` | все четыре | DeFi | Перевести |
| `asset` | Пополнить («Приобрести») | продажа | Пополнить |
| `liability` | Оплатить, Перевести | — | Перевести |

### Валидация (V1–V14)

Проверяется при каждом `add_transaction` / `update_transaction`
(`service/validate.rs`); ошибка — `INVALID_INPUT` с текстом для формы.
Узлы существуют и не в архиве; вид разрешён для каждого узла; `cash`/`bank`
держат только фиат, `asset` — только вещи; суммы > 0; у Пополнить/Оплатить есть
категория подходящей группы (Пополнить — доход или служебная, Оплатить — не
доход), не архивная; у перевода и обмена ≥ 1 своя сторона и разные стороны,
у обмена — разные активы; реквизит контакта принадлежит ему и активен; детали —
только допустимые для типа; `closed_pnl` — только при закрытии; дата валидна и
не дальше суток вперёд; направление долга совпадает со сторонами. Отрицательный
баланс (V13) — не ошибка.

Перед проверкой категории нормализуются, а пустые заполняются первым
подходящим правилом (`CategorizationRule`: подстрока комментария или имени
контакта, без учёта регистра).

## Контакты

`Contact { id, name, kind (person|company|exchanger), accounts, note, archived, created_at }`,
`ContactAccount { id, group?, label, kind (iban|account|crypto_address|card|phone|url|other), value, currency?, network?, active }`.
Обменник — контакт `exchanger`, обмен через него — `Convert` с `meta.contact_id`.

- Удаление контакта, на который ссылаются операции или кошельки, — ошибка
  («архивируйте»). Убранный реквизит, по которому были операции, не удаляется,
  а становится `active: false`.
- Обороты (`contact_turnover`) — отправлено / получено по активам, без внутренних
  перемещений.
- Долги — `TransferTx.debt`: `lend` (дал в долг), `collect` (мне вернули),
  `borrow` (взял), `repay` (вернул). `contact_balances()` — «мне должны / я
  должен» по контакту и активу. Долги не доход и не расход.

## Категории

`Category { key, name?, group, archived }`. `key` неизменяем и хранится в
операциях; `name: None` — подпись из i18n `categories.default.<key>`. Группы:
`income`, `fixed` (обязательные), `daily` (ежедневные), `savings`, `service`
(служебные — вне бюджетов). Удалить можно только неиспользуемую категорию.

## Капитал, ручная цена, классификация

- `net_worth()` → `NetWorth { financial, non_financial, receivables, liabilities, payables, net }`
  в базовой валюте; `PortfolioSummary` несёт `assets_base`, `liabilities_base`,
  `net_worth_base`.
- `ManualPrice { asset_key, unit_price, currency, updated_at }` — оценка вещи в
  узле `asset`, действует на всю историю (D24). Без неё — цена покупки.
- `classify_flow(tx) → FlowClass` (`income`, `fixed`, `daily`, `savings`,
  `budget_in`, `budget_out`, `ignored`) — общая основа бюджетов, cashflow и
  итогов месяца (план §7.3): учитывает `in_budget`, группы категорий,
  платежи по кредитам, переводы в узлы целей, внутренние и долговые переводы.

## Деньги: Amount

`Amount` — newtype над `rust_decimal::Decimal`:
- всегда **сериализуется строкой** (точность, стабильный JSON и bincode);
- `Amount::parse("123.45")`, `Amount::from_f64`, `to_f64()` (только для отображения/engine),
- арифметика Decimal (без плавающих ошибок), `round(n)`;
- в `ipc.gen.d.ts` денежные поля объявлены как `type Amount = string;`.

## Кэши цен — история курсов

- `PricePoint { asset_key, value_usd, ts, source }` — один юнит актива в USD.
- `FxPoint { currency, per_usd, ts, source }` — единиц валюты за 1 USD.
- Это **история**, а не последняя точка: на валюту/актив — одна точка в час,
  последняя неделя почасово, старше — одна в день, максимум 2000 точек
  (`pricing/history.rs`).
- `refresh_all` приносит всю таблицу FX; историю хранят только валюты, которыми
  пользуется vault (USD, базовая, валюта бюджета, фиат в операциях, валюты
  ручных цен), у остальных — последняя точка.
- Курс на момент — ближайшая точка ≤ момента, иначе первая после. Фиат стоит
  `1 / per_usd`; ручная цена имущества действует на всю историю.
- Кросс-курс A↔B — через USD (`convert_amount`).

### Штамп цены

При сохранении операции каждая сторона без цены получает `price_usd` по курсу
на дату операции; введённое руками не трогается. `price_base` не
записывается: стоимость в базовой валюте считается как `price_usd × курс базы
на дату операции`, поэтому смена базовой валюты ничего не ломает. Нет курса —
операция сохраняется с пометкой «без цены» и получает цену при следующем
обновлении курсов (`restamp_missing`).

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

`ExchangeKeys` получил `spot_store_id?` и `futures_store_id?` — субсчета, на
которые записываются подтверждённые спот- и фьючерсные сделки (по умолчанию —
сам кошелёк ключа). `PendingImport` хранит `is_maker`, `closed_size`,
`closed_pnl`; фьючерсная сделка с `closed_size > 0` — закрытие позиции.

В `VaultDoc` добавлено поле `exchanges: Exchanges` (схема версии **1**, формат
сброшен с v2 на v1 — обратная совместимость не сохраняется).

```rust
pub struct ExchangeKeys {
    pub store_id: String,  // ID кошелька (узла-биржи), которому принадлежит ключ
    pub api_key: String,
    pub api_secret: String,
    pub spot_store_id: Option<String>,    // куда писать спот-сделки
    pub futures_store_id: Option<String>, // куда писать фьючерсы
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
