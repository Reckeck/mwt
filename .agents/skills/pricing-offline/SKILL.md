---
name: pricing-offline
description: >
  Прайсы, FX, провайдеры и офлайн-деградация в trade-app: PricingService,
  цепочка фолбэков CoinGecko → CoinCap → Binance, open.er-api для валют, история
  курсов внутри vault, штамп цены операции и правило «сеть от кэша отделена». Использовать при правках
  crates/core/src/pricing/**, commands/pricing.rs, model/vault.rs (price_cache,
  fx_cache), а также при задачах «цена не обновляется», «офлайн», «stale price»,
  «курс валюты», «новый провайдер», «кэш», «refresh_all», «тормозит при вводе
  цены», «429», «rate limit». Триггеры: pricing, price, prices, цена, котировки,
  FX, курс, валюта, USD, CoinGecko, CoinCap, Binance, er-api, TTL, cache, кэш,
  offline, офлайн, деградация, refresh_all, PricePoint, FxPoint, price_cache,
  fx_cache, search_assets, get_cached_price.
---

# Прайсы, FX и офлайн-деградация

`crates/core/src/pricing/mod.rs` — единственный модуль, ходящий в сеть по ценам.
Все эндпоинты бесплатные и **без ключей**.

## Провайдеры

| Что | Цепочка |
|---|---|
| Крипта (spot) | CoinGecko (`/simple/price` по id) → CoinGecko (токен по контракту) → CoinCap → Binance ticker |
| Токены ERC-20 и т.п. | CoinGecko `/simple/token_price/{platform}` (нужны `chain` + `contract`) |
| Валюты (FX) | `open.er-api.com/v6/latest/USD` → Frankfurt/ECB |

Константы вверху файла: `COINGECKO_V3`, `COINCAP_V2`, `BINANCE_API`, `ER_API`.
Fallback-цепочка живёт в `price_usd(&self, asset)` — **четыре последовательные
попытки, каждая на `if let Ok(...)`**. Упавшая попытка молча уходит к
следующей; ошибка наружу уходит только если не сработала ни одна.

## История курсов внутри vault (план §6.1)

- Кэш лежит **в vault**: `doc.price_cache: Vec<PricePoint>` и
  `doc.fx_cache: Vec<FxPoint>`, то есть зашифрован и переживает рестарт.
  Это не in-memory кэш — сброс приложения кэш не обнуляет.
- Кэш — **история**, а не «последняя точка»: `pricing/history.rs`,
  `merge_prices` / `merge_fx`. На серию (валюта или ключ актива) — одна точка
  в час (свежая побеждает), последняя неделя (`HOURLY_KEEP`) почасово, старше —
  одна точка в день, не больше `HISTORY_CAP = 2000` точек.
- `fetch_fx()` приносит **всю таблицу** er-api. Историю хранят только валюты из
  `UnlockedVault::fx_currencies_in_use()` (USD, базовая, валюта бюджета, фиат в
  операциях и холдингах, валюты ручных цен); остальные — одну последнюю точку,
  иначе ~170 валют почасово раздули бы vault, который перешифровывается на
  каждую запись. Следствие: у валюты, которой начали пользоваться сегодня,
  прошлого нет.
- Время сравнивается по unix-секундам (`history::unix_secs`), не строкой:
  `…:00Z` и `…:00.5Z` строкой сортируются наоборот.
- Запросы «на момент» — через `RateBook` (`UnlockedVault::rate_book()`,
  `service/rates.rs`): кэш разбирается один раз, дальше бинарный поиск.
  `fx_at` / `price_at` — ближайшая точка ≤ ts, иначе первая после;
  `history_price` для графика сначала смотрит прошлое (кэш, штампы операций) и
  лишь потом будущее; `convert` — кросс-курс через USD; `leg_price_in_base` —
  цена проводки в базовой валюте по курсу дня операции.
- Фиат оценивается через FX: `fiat:RUB` → `1 / per_usd(rub)`. Ручная цена
  имущества (D24) действует на всю историю. Деривативы цены не имеют.

## Штамп цены при сохранении (план §6.2)

`add_transaction` / `update_transaction` вызывают `stamp_prices`: каждая сторона
без `price_usd` и `price_base` получает `price_usd = price_at(актив, timestamp)`.
Введённое руками не перезаписывается. `price_base` штамп **не** пишет —
базовая валюта меняется, а курс к ней берётся из истории FX на дату операции.
Нет курса → операция сохраняется без цены; `refresh_all` после применения
курсов зовёт `restamp_missing()` и пишет число в `RefreshResult.restamped`.

## Железное правило: сеть отделена от мутации документа

Это архитектурное правило №4 проекта, и оно не про стиль, а про вендинг:

```rust
// СЕТЬ — чистые, не трогают doc, безопасны для .await
async fn fetch_prices(&self, assets: &[AssetRef]) -> Result<Vec<PricePoint>>
async fn fetch_fx(&self)                            -> Result<Vec<FxPoint>>

// МУТАЦИЯ — применяют уже полученное (не персистят: refresh пишет один раз)
fn UnlockedVault::apply_prices(&mut self, fresh: &[PricePoint])
fn UnlockedVault::apply_fx(&mut self, fresh: &[FxPoint])   // знает, чью историю хранить
fn UnlockedVault::restamp_missing(&mut self) -> usize
```

**Почему так:** `doc` лежит внутри `Mutex<Option<UnlockedVault>>`, а Guard
нельзя держать через `.await`. Чистый `fetch_*` позволяет забрать из vault
нужные данные, отпустить Guard, спокойно awaited-ить сеть, взять Guard снова и
применить. Эталон — `refresh_all` в `crates/app/src/commands/pricing.rs`:

```rust
let assets = { let g = require_vault(&state)?; /* …копия… */ };
let prices = state.pricing.fetch_prices(&assets).await?;          // await без guard
let fx     = state.pricing.fetch_fx().await?;
let mut g = require_vault(&state)?;                                // guard снова
let vault = g.as_mut().unwrap();
vault.apply_prices(&prices);
vault.apply_fx(&fx);
let restamped = vault.restamp_missing();
vault.persist()?;
```

Никогда не пиши `&mut doc` и `.await` в одной функции. Если кажется, что
«нужно сначала проверить кэш, потом догрузить» — это делается **снаружи**,
копированием `Vec<AssetRef>`/имени базовой валюты.

## Офлайн: что происходит без сети

Отдельные команды **не падают** при сетевом сбое — возвращают пустую выборку с
уведомлением, оставляя состояние как есть:

| Команда | Поведение при сбое |
|---|---|
| `get_cached_price` / `convert_amount` | cache-only, сети не касаются — безопасно звать на каждый ввод; `at` — момент, на который нужен курс |
| `search_assets` / `inspect_tx` / `bybit_executions` | пустой список + `notice` (`empty`/`unavailable`/`choose_network`) |
| `refresh_all` | ошибка наружу (`errors.NETWORK`/`errors.PRICING`), кэш **не трогается** |

Форма сделки при этом не сбрасывается — то, что пользователь набрал руками,
остаётся на месте. Это осознанное требование, не баг: не «сеть от кэша
отделена» должно означать «не потерять ввод».

## Антипаттерны

- **Не делай запрос на цену в обработчике `input`/на каждый ввод.** Это
  гарантированный rate-limit. Для показа подсказки цены есть
  `get_cached_price` (без сети). Реальные запросы — по кнопке refresh или по
  таймеру с дебаунсом.
- **Не гоняй один HTTP-запрос на актив.** `fetch_prices` делает
  `price_usd(asset).await` **последовательно по каждому активу**, а внутри
  `price_usd` — до четырёх попыток. На 50 активах это до 200 запросов.
  `refresh_all` дедуплицирует активы по `asset.key()` и фильтрует фиат, но
  батчить всё равно правильнее.
- **`fetch_prices` пробрасывает ошибку первого упавшего актива** (`?` внутри
  цикла), то есть частичного успеха не бывает: весь батч падает. Если
  требуется «посчитать что успели» — это отдельная задача, меняй явно и
  покрывай тестом.
- **Не пиши цены и курсы числами.** Цена — `Amount` (Decimal); в TS приходит
  строкой. `f64` допустим только в `market/` (свечи для графиков, см. скилл
  `vault-crypto`).
- **Не хардкодь `source`.** Он идёт в `PricePoint`/`FxPoint` и виден в данных
  vault: `"cryptoprice"` / `"open.er-api"` — существующие значения.
- **Не добавляй ключ API.** Вся схема построена на keyless-эндпоинтах;
  секретный ключ потребует новой ветки секретов в vault (см. `vault-crypto`).

## Событие

`refresh_all` после успеха эмитит `prices-updated` с `RefreshResult`. Фронт
подписывается через `onEvent<T>` из `src/shared/api/ipc.ts` и инвалидирует
query-ключи (`['summary']`, `['store', id]`, `['store-history', id]`); при
`restamped > 0` `AppShell` инвалидирует ещё и списки операций — бейдж «без
цены» уходит.
Новые события — регистрируй здесь же и в `docs/ipc.md`.

## Тесты и проверки

Юнит-тесты истории — `pricing/history.rs`, штампа и курсов на дату —
`service/rates.rs`; все без сети. Всё, что ходит в HTTP,
проверяется **только вручную**: `mise run dev`, затем `refresh_all` в UI.

```bash
mise run test        # не сломать чистую логику кэша
mise run typecheck   # если менял DTO
mise run check       # перед коммитом
```

Если менял `price_cache`/`fx_cache` в `model/vault.rs` — это **схема vault**:
версию **не бампать**, поле аддитивное через `#[serde(default)]` (скилл
`vault-crypto`, AGENTS.md правило 5). Если тип попадает в IPC — `mise run gen-types`.