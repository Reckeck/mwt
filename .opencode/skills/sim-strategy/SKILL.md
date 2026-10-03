---
name: sim-strategy
description: >
  Стратегии и симулятор в trade-app: модель Strategy/Rule, реплей свечей,
  индикаторы SMA/EMA/RSI, offline-GBM генератор, метрики SimRun и изолированный
  бумажный аккаунт. Использовать при правках crates/core/src/sim/**,
  commands/sim.rs, Strategy Lab UI, а также при задачах «новый индикатор»,
  «новое правило входа/выхода», «изменить метрики», «стратегия не сигналит»,
  «реплей», «синтетические данные», «бумажный аккаунт». Триггеры: sim,
  simulator, strategy, стратегия, backtest, бэктест, replay, реплей, candle,
  свеча, OHLC, SMA, EMA, RSI, GBM, synthetic, equity curve, drawdown, просадка,
  win rate, entry_rules, exit_rules, SimRun, DataSource, paper_trade.
---

# Симулятор и Strategy Lab

`crates/core/src/sim/` — чистый Rust, без Tauri. `mod.rs` — модель,
`engine.rs` — реплей. UI — `src/views/StrategyLabView.vue`.

## Границы модуля

- **Никаких реальных денег и реальных ордеров.** Приложение принципиально не
  умеет выставлять ордера — в клиенте Bybit только GET-эндпоинты. Симулятор
  не делает исключения из этого правила.
- Бумажный аккаунт (`StoreKind::Paper`) **изолирован** от реальных хранилищ:
  стратегии пишут обороты в `SimRun`/`SimTrade`, а не в `doc.transactions`.
  Не «упрощай» это слиянием — портфель и бумажный счёт не должны смешиваться.
- Ранн хранится в vault: `doc.sim.strategies`, `doc.sim.runs`. Лимит — **200
  последних раннов** (`runs.truncate(200)`), новые кладутся в начало
  (`insert(0, …)`).
- Удаление стратегии каскадно чистит её ранны (`runs.retain(|r| r.strategy_id != id)`).

## Модель (`sim/mod.rs`)

| Тип | Назначение |
|---|---|
| `DataSource` | `Real` (Binance klines, публичный REST) \| `Synthetic` (offline GBM) |
| `Rule` | `{ indicator: String, field: String, op: String, value: Amount }` |
| `Strategy` | `id`, `name`, `symbol`, `chain`, `source`, `initial_capital`, `asset`, `entry_rules`, `exit_rules`, `stop_loss_pct`, `take_profit_pct`, `position_size_pct`, `created_at` |
| `SimTrade` | `ts`, `side` (`"buy"`/`"sell"`), `price`, `qty`, `value`, `fee` |
| `SimRun` | метрики + `equity_curve: Vec<EquityPoint>` + `trades` + `synthetic: bool` |
| `EquityPoint` | `{ ts, equity, value }` |

`Rule.indicator` и `Rule.op` — **строки**, не енамы. Это сознательный
компромисс: значения приходят из DTO и редактируются в UI-формах без
перегенерации схемы. Следствие: опечатка в строке **не ловится компилятором** —
`engine.rs` обязан деградировать (правило просто не срабатывает), а не паниковать.

## Правило `f64` в движке

Единственное место в проекте, где `f64` разрешён — **`sim/engine.rs`**, и
только потому, что это численный реплей.

- Вход и выход движка — `Amount` (Decimal). Конверсия только на границе:
  `Amount::to_f64()` внутрь, `Amount::from_f64()` наружу
  (`EquityPoint::new`, `SimTrade`, метрики `SimRun`).
- **`f64` не должен утёкать за пределы `engine.rs`.** Если сигнатура новой
  функции требует `f64` и её вызывает `crates/app` — переделай на `Amount`.
- Индикаторы возвращают `Vec<Option<f64>>` — `None` на позициях, где окно
  ещё не набрано (`Vec::with_capacity` + заполнение с `period`).
- Неявное округление запрещено: округляй явно, `Amount::round(dp)`.

**Edition 2024:** `gen` и `await` — зарезервированные слова. Генератор
использует `rand::random::<f64>()`, и называть переменную/метод `gen` нельзя.

## Индикаторы и реплей

- Индикаторы: `sma(values, period)`, `ema(values, period)`, `rsi(values, period)`
  — все `pub fn` в `engine.rs`, все чистые и детерминированные.
- `series_for(name, values)` — диспетчер индикаторов по строковому имени из
  `Rule.indicator`; `eval_rule(rule, cur, prev)` и `all_rules(rules, series, i)`
  — исполнение правил. **Добавляешь индикатор → добавляй ветку в
  `series_for`**, иначе UI покажет индикатор, а движок его не увидит.
- `run_on_candles(strategy, candles) -> SimRun` — главная точка входа движка.
  Держит внутренне `cash_f: f64`, `qty: f64`, трекит `max_equity`/`max_dd` для
  просадки.
- `synthetic_candles(symbol, n, start, vol, per_year)` — offline GBM через
  `standard_normal()`; `high`/`low` достраиваются от `open`/`close`.
  Существует, чтобы Strategy Lab работал **без сети** — не делай
  `DataSource::Synthetic` сетевым.
- `interval_str(min)` — маппинг гранулярности в подпись биржи.

## Сеть и деградация

`fetch_candles(symbol, granularity_min, candles, client)` ходит в публичный
Binance klines. Обработка входных данных — в `commands/sim.rs`:

- `granularity_min.max(1)`, `candles.clamp(50, 5000)`, `n = candles.max(50)` —
  **входные границы нормализуются в хендлере**, а не в движке. Движок доверяет
  этим значениям.
- Таймаут клиента 20 с. Ошибка сети возвращается как `err_to_string` →
  локализуется по коду (`errors.NETWORK` и т.п.).
- **Правило трёх шагов (обязательно, как везде в этом проекте):** в
  `run_strategy` стратегия копируется из vault в локальную переменную, guard
  **отпускается**, и только потом идёт `.await` на `fetch_candles`, затем guard
  берётся снова для сохранения ранна. `MutexGuard` нельзя держать через
  `.await` — future перестанет быть `Send`.

## Тесты

В `engine.rs` есть `#[cfg(test)] mod tests`:

- `sma_ema_rsi_basic` — индикаторы на известном ряде;
- `uptrend_yields_positive_return` — восходящий тренд даёт положительную доходность;
- `synthetic_generates_candles` — генератор выдаёт корректные свечи.

При изменении движка добавляй тест сюда же. Критичные случаи:
пустой ряд, ряд короче периода индикатора, правило с неизвестным индикатором,
`stop_loss`/`take_profit` на границе, нулевой `position_size_pct`.

## Изменения на frontend

Команды: `save_strategy`, `list_strategies`, `delete_strategy`, `list_runs`,
`run_strategy` (см. `docs/ipc.md`). Query-ключи — `['strategies']`, `['runs']`.

Если добавил поле в `Strategy`/`SimRun`:
1. `#[derive(ts_rs::TS)]`, если тип попадает в IPC;
2. `mise run gen-types` — проверь дифф `src/lib/ipc.gen.d.ts`;
3. новые UI-строки — **в оба локаля** (`sim.*` в `ru.json` и `en.json`);
4. `mise run check`.

Документация — `docs/` (см. раздел по стратегиям) и `README.md`.
Коммит — только по явной просьбе пользователя (скилл `jj-workflow`).