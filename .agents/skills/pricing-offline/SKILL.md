---
name: pricing-offline
description: >
  Прайсы, FX, провайдеры и офлайн-деградация в trade-app: PricingService,
  цепочка фолбэков CoinGecko → CoinCap → Binance, open.er-api для валют, TTL-кэш
  внутри vault и правило «сеть от кэша отделена». Использовать при правках
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

## TTL-кэш внутри vault

```rust
pub const PRICE_TTL_SECONDS: i64 = 5 * 60;   // 5 минут
pub const FX_TTL_SECONDS:    i64 = 60 * 60;  // 1 час
```

- Кэш лежит **в vault**: `doc.price_cache: Vec<PricePoint>` и
  `doc.fx_cache: Vec<FxPoint>`, то есть зашифрован и переживает рестарт.
  Это не in-memory кэш — сброс приложения кэш не обнуляет.
- Свежесть проверяет `fresh(iso_ts, ttl_secs)` — парсит RFC3339 через `time`,
  **при ошибке парсинга возвращает `false`** (считается несвежим). Это
  безопасный дефолт: битый таймстамп не даёт верить мусорным данным.
- `upsert_prices` / `upsert_fx` — `retain` по ключу + `extend` свежими, то есть
  **последняя точка заменяет прежнюю, а не добавляется**. Плюс prune: если по
  активу накопилось больше 2000 точек, отбрасывается старейшая половина.
- `price_usd_cached` / `fx_rate` — «прочитал из кэша; если пусто или протухло —
  сходи в сеть и запиши обратно». Именно ими пользуются калькулятор и формы.

## Железное правило: сеть отделена от мутации документа

Это архитектурное правило №4 проекта, и оно не про стиль, а про вендинг:

```rust
// СЕТЬ — чистые, не трогают doc, безопасны для .await
async fn fetch_prices(&self, assets: &[AssetRef]) -> Result<Vec<PricePoint>>
async fn fetch_fx(&self, base: &str)               -> Result<Vec<FxPoint>>

// МУТАЦИЯ — применяют уже полученное
fn apply_prices(doc: &mut VaultDoc, fresh: &[PricePoint])
fn apply_fx(doc: &mut VaultDoc, fresh: &[FxPoint])
```

`refresh_prices` / `refresh_fx` — это просто `fetch_*` + `apply_*`, и они
существуют для удобства **там, где мьютировать doc не нужно**.

**Почему так:** `doc` лежит внутри `Mutex<Option<UnlockedVault>>`, а Guard
нельзя держать через `.await`. Чистый `fetch_*` позволяет забрать из vault
нужные данные, отпустить Guard, спокойно awaited-ить сеть, взять Guard снова и
применить. Эталон — `refresh_all` в `crates/app/src/commands/pricing.rs`:

```rust
let (assets, base) = { let mut g = require_vault(&state)?; /* …копия… */ (assets, base) };
let prices = state.pricing.fetch_prices(&assets).await?;          // await без guard
let fx     = state.pricing.fetch_fx(&base).await?;
let mut g = require_vault(&state)?;                                // guard снова
PricingService::apply_prices(&mut g.as_mut().unwrap().doc, &prices);
PricingService::apply_fx(&mut g.as_mut().unwrap().doc, &fx);
```

Никогда не пиши `&mut doc` и `.await` в одной функции. Если кажется, что
«нужно сначала проверить кэш, потом догрузить» — это делается **снаружи**,
копированием `Vec<AssetRef>`/имени базовой валюты.

## Офлайн: что происходит без сети

Отдельные команды **не падают** при сетевом сбое — возвращают пустую выборку с
уведомлением, оставляя состояние как есть:

| Команда | Поведение при сбое |
|---|---|
| `get_cached_price` | cache-only, сети не касается — безопасно звать на каждый ввод |
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
  строкой. `f64` допустим только в `sim/engine.rs` (см. скилл `vault-crypto`
  и `sim-strategy`).
- **Не хардкодь `source`.** Он идёт в `PricePoint`/`FxPoint` и виден в данных
  vault: `"cryptoprice"` / `"open.er-api"` — существующие значения.
- **Не добавляй ключ API.** Вся схема построена на keyless-эндпоинтах;
  секретный ключ потребует новой ветки секретов в vault (см. `vault-crypto`).

## Событие

`refresh_all` после успеха эмитит `prices-updated` с `RefreshResult`. Фронт
подписывается через `onEvent<T>` из `src/lib/ipc.ts` и инвалидирует
query-ключи (`['summary']`, `['store', id]`, `['store-history', id]`).
Новые события — регистрируй здесь же и в `docs/ipc.md`.

## Тесты и проверки

Юнит-тесты `pricing/mod.rs` — без сети (2 теста). Всё, что ходит в HTTP,
проверяется **только вручную**: `mise run dev`, затем `refresh_all` в UI.

```bash
mise run test        # не сломать чистую логику кэша
mise run typecheck   # если менял DTO
mise run check       # перед коммитом
```

Если менял `price_cache`/`fx_cache` в `model/vault.rs` — это **схема vault**:
нужны бамп `VAULT_FORMAT_VERSION`, миграция и тест на старый файл (скилл
`vault-crypto`). Если тип попадает в IPC — `mise run gen-types`.