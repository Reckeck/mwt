# Changelog

Все значимые изменения проекта. Формат основан на
[Keep a Changelog](https://keepachangelog.com/ru/1.1.0/), версионирование —
[Semantic Versioning](https://semver.org/lang/ru/).

Формат файла vault: `*.wallet` (зашифрованный контейнер), схема payload —
`VAULT_FORMAT_VERSION` (`crates/core/src/lib.rs`). Обратная совместимость
снимков **не гарантируется**: файл, записанный другой версией схемы, будет
отклонён с `UNSUPPORTED`.

## [Unreleased]

Фаза D: торговый терминал, Strategy Lab с объяснением сигналов, импорт
истории Bybit со staging-подтверждением, а также проход по дизайн-системе,
валидация хешей транзакций и список недавних хранилищ.

### Added

#### Торговый терминал (D2.1)

- `src/components/charts/TradingChart.vue` — свечной терминал на
  lightweight-charts 5: таймфреймы 1/3/5/15/30/60/240/1440 мин, индикаторы
  SMA20 / EMA50 / RSI14 (RSI — в отдельной панели), панель объёма, легенда
  OHLC, водяной знак символа, индикатор «Live / Connecting / Offline».
- Рисовалка: трендовая линия, горизонтальная линия, уровни Фибоначчи,
  прямоугольник, кисть, текст, стрелка; режим «магнит», отмена, очистка.
  Рисунки сохраняются в `localStorage` под ключом
  `tradeapp.drawings.<SYMBOL>` и переживают перезапуск.
- Онлайн-данные: прямое подключение к Binance WebSocket
  (`wss://stream.binance.com:9443/ws/<pair>@kline_<tf>`) с реконнектом;
  при обрыве UI показывает «Офлайн» и продолжает работать на последних
  загруженных свечах.
- `src/views/TradingView.vue` — экран терминала: поле символа и быстрый
  выбор из списка (BTC, ETH, SOL, BNB, XRP, DOGE, ADA, AVAX).
- Маршрут `/trading`, пункт в боковой навигации и в трее.
- Новая команда `get_candles` (`GetCandlesInput` → `GetCandlesOutput`) —
 — thin-обёртка над `sim::engine::fetch_candles`, работает без разблокированного
  хранилища.
- Зависимости: `lightweight-charts@5.2.1`, `lightweight-charts-drawing@0.4.0`.

#### Strategy Lab: график сделок, сигналы и GBM-прогноз (D2.2)

- `SimRun` расширен полями `signals`, `candle_params`, `granularity_min`.
- `SimSignal` — сработавший сигнал: время, сторона (`entry` / `exit`), причина
  (`rules` / `stop_loss` / `take_profit` / `liquidation`), текстовое объяснение
  сработавшего правила и цена.
- Сигналы пишутся прямо в движке: `run_on_candles` теперь объясняет каждое
  срабатывание (например `sma20(25.3100) gt 25`), а не просто выполняет его.
- Синтетические свечи стали воспроизводимыми: `synthetic_candles` принимает
  `seed` и `step_min`, использует `StdRng` и фиксированную эпоху отсчёта
  (2026-01-01T00:00:00Z). Один и тот же seed — та же серия, поэтому маркеры
  сделок и сигналов совпадают с графиком при повторном открытии.
- `CandleParams` — параметры генератора (`seed`, `n`, `start_price`,
  `annual_vol`, `candles_per_year`), сохраняются в прогоне как ключ реплея.
- Новый движковый API: `synthetic_from_params`, `realized_sigma` (σ по
  логарифмическим доходностям), `gbm_paths` (бездрейвиговые пути),
  `fetch_candles_between` (исторический диапазон с пагинацией),
  `rfc3339_to_millis`, `iso_from_millis`.
- Новые команды: `get_run_candles` (восстанавливает свечи прогона: синтетика —
  по seed, реальные — догружает диапазон с Binance) и `get_gbm_paths`
  (веер прогноза от последней цены прогона; σ берётся из `candle_params`
  либо из realized vol на реальных свечах).
- `src/views/StrategyLabView.vue` — выбор прогона, свечной график с маркерами
  покупок/продаж и маркерами входа/выхода, таблица сигналов с причинами,
  переключатель «веера» GBM, отдельный график кривой доходности.
- `echarts.ts` теперь регистрирует `CandlestickChart`, `ScatterChart` и
  `DataZoomComponent`.

#### Импорт истории Bybit со staging-подтверждением (D2.4)

- `PendingImport` в `VaultDoc.pending_imports` — сделка, ожидающая
  подтверждения. Хранит обе ноги, `execId` (ключ дедупликации), комиссию и
  время в ISO-8601; денежные поля — `Amount`.
- `PendingImport::from_execution` превращает fill Bybit в staging-запись и
  возвращает `None` для негодных сделок: без `execId`, с нечитаемым
  количеством или ценой, с нулевым количеством. Мусор не доходит до
  операций пользователя.
- Методы `UnlockedVault`: `stage_imports` (идемпотентно по `execId`,
  лимит 10 000), `pending_imports` (новые сверху), `confirm_pending_imports`
  (превращает staging в сделки — две ноги, как `add_trade`, с общей
  `group_id`), `discard_pending_imports`.
- Команды: `import_bybit_history`, `list_pending_imports`,
  `confirm_pending_imports`, `discard_pending_imports`.
- `Execution.exec_id` + `Execution::category_hint()`; `BybitClient::all_executions`
  — пагинация истории окнами по 7 дней (`/v5/execution/list` не поддерживает
  `cursor` и запрещает окна шире 7 дней), до 12 страниц на категорию.
- Фоновая проверка: поток в `crates/app/src/lib.rs` просыпается каждые 30 с,
  читает `settings.bybit_import_interval_sec` (`0` — выключено, иначе
  клампится в 60…86400) и складывает новые сделки в staging. Событие
  `bybit-import-pending` ({ staged }) показывает баннер в шапке.
- UI: в карточке Bybit на экране кошелька — кнопка «Загрузить историю»,
  счётчик ожидающих, таблица со строками и действиями «подтвердить» /
  «отклонить» по каждой сделке и разом.

**Инвариант:** ни одна команда и ни один фоновый опрос не создают операции
сами — только явное подтверждение пользователя.

#### Дизайн-система и настройки интерфейса (D1)

- `src/lib/store-icons.ts` — иконки по типу кошелька (используются в
  `SidebarWallets`, `StoresView`, `WalletView`), `src/components/ui/tooltip.vue` —
  примитив подсказки (AppShell, терминал), `src/lib/spacing.ts` — шкала отступов
  (заготовка под следующий проход по вёрстке).
- `src/views/DashboardView.vue`, `StoresView.vue`, `ChartsView.vue`,
  `SidebarWallets.vue`, `TransactionsView.vue` переведены на общие токены.
- Настройка состава виджетов дашборда (`settings.dashboard_widgets`), избранные
  валюты в шапке, интервал обновления цен, сворачивание в трей, индикатор
  «Оффлайн».
- Раздел «Для разработчика» в настройках: открыть vault и скачать его JSON.
- `docs/ui-design.md` — правила дизайн-системы; скилл
  `.opencode/skills/ui-design-system`.

#### Недавние хранилища

- Команды `recent_vaults`, `select_vault`, `forget_recent_vault`,
  `pick_vault_file` и список недавних файлов в разблокировке.
- `UnlockView` — два шага: выбор из списка недавних (с пометкой «существует» /
  «файл не найден» / «старый формат») или свой путь с системным диалогом;
  кнопка «Назад к списку».
- Пустой пароль отклоняется на входе (`INVALID_INPUT`) — иначе файл
  создавался бы незашифрованным по факту пустого пароля.

#### Валидация хешей транзакций (D2.3)

- Новые коды ошибок `TX_HASH_MALFORMED` и `TX_HASH_NETWORK_MISMATCH`:
  проверяется форма хеша и его соответствие сети актива, а не только
  непустота.

### Changed

- `storage::decode` и `decode_bin` больше не молча принимают усечённые
  bincode-снимки: `decode_bin` проверяет, что payload потреблён целиком
  (`n == bytes.len()`), иначе — `DESERIALIZE`. Версия схемы не поднята,
  миграции v1→v2 нет: несовместимый файл отклоняется явно.
- `sim::engine::run_on_candles`: правила вычисляются через `rules_detail`,
  который возвращает описание совпавших условий; приоритет у стоп-лосса и
  тейк-профита, если оба условия выполнены на одной свече.
- `rules_detail` требует наличия предыдущего значения индикатора, поэтому
  поведение на «разогреве» индикаторов не изменилось.
- Денежные поля `PendingImport` — `Amount`, а не строки; лимит staging — 10 000 записей.
- `ui-design-system`, `docs/ui-design.md`, план по хранилищам и обменам —
  документация в репозитории.
- В `ipc.gen.d.ts` добавлены типы `PendingImport`, `SimSignal`, `CandleParams`,
  `GetRunCandles*`, `GbmPath*`, `ImportBybitHistory*`, `Confirm/DiscardPendingImports*`,
  `RecentVault`, `DevDumpVaultOutput`, `BybitExecution*`.

### Fixed

- `clippy -D warnings` по всему воркспейсу: `collapsible_if` в
  `service.rs` переведён на let-цепочки, убраны неиспользуемые импорты в
  `storage/mod.rs`, `vault_recents.rs`, `settings.rs`.
- `fetch_candles_between` дублирует соседние свечи на границах страниц —
  добавлена дедупликация по `ts`.
- Мёртвый код пагинации по `cursor` в `all_executions` заменён на оконную
  пагинацию (Bybit игнорирует `cursor` для списка исполнений, из-за чего
  страницы повторялись бы до достижения лимита).

### Прочее

- Трей и боковая навигация пополнились пунктами «Терминал» и «Лаборатория».
- Все пользовательские тексты добавлены в оба локали (ru/en) — 97 новых
  ключей, наборы ключей синхронизированы.
- `docs/ipc.md` дополнен разделами по реплею прогонов и импорту Bybit;
  `docs/data-model.md` — новые сущности.

### Проверки

`cargo fmt`, `cargo clippy --workspace -- -D warnings`, `mise run check`
(eslint + vue-tsc + 81 тест воркспейса), `pnpm exec vite build` — зелёные.

[Unreleased]: https://example.invalid/trade-app/compare/v0.1.0...HEAD