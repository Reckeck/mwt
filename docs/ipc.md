# IPC-контракт (frontend ↔ backend)

Все команды — `#[tauri::command]` в `crates/app/src/commands/`. Типы — из
`crates/core/src/ipc_contract.rs` и домена core, генерируются ts-rs в
`src/shared/api/ipc.gen.d.ts` через `mise run gen-types`.

Ошибки: команды возвращают `Result<T, String>`; строка —
`ERR_CODE: message`, где `ERR_CODE` — метод `Error::code()`
(например `CRYPTO`, `MISSING`, `INVALID_INPUT`, `REQUIRES_UNLOCK`, `NOT_FOUND`).
Frontend парсит код и показывает локализованное сообщение.

## События

| Событие | Payload | Когда |
|---|---|---|
| `prices-updated` | `RefreshResult` | после `refresh_all` и `ensure_fx` (если что-то найдено) |
| `bybit-import-pending` | `{ staged: number }` | фоновый опрос Bybit нашёл новые сделки (D2.4) |
| `automation-created` | `{ tx_ids: string[] }` | планировщик создал операции автоматизаций |
| `automation-rejected` | `AutomationRejectedEvent { run_id, automation_id, name, planned_at, reason }` | запуск не создан: не хватило средств, лимит кредитки или операция не прошла проверку (D26); тост с «Повторить / Пропустить» + системное уведомление |
| `automation-skipped` | `{ count }` | после долгого перерыва старые даты сверх 24 записаны как пропущенные |
| `price-alert` | `{ alert_id }` | сработал ценовой алерт (один раз) |
| `reminder-due` | `{ id }` | наступило время напоминания (один раз за сессию) |
| `enrichment-ready` | `{ tx_id }` | обозреватель вернул данные по хешу операции; находка ждёт «Применить / Отклонить» |

Фоновые задачи живут в `crates/app/src/scheduler.rs`: один поток, тик раз в 30 с,
только при разблокированном vault — импорт Bybit, автоматизации, алерты (цены их
активов — не чаще раза в 5 минут), напоминания. Системные уведомления —
`tauri-plugin-notification` (capability `notification:default`).

## Команды

### auth (`crates/app/src/commands/auth.rs`)
| Команда | Вход | Выход |
|---|---|---|
| `vault_status` | — | `VaultStatus { exists, unlocked, path, format }` |
| `create_vault` | `CreateVaultInput { path, password, cash_name? }` | `VaultInfo` — vault создаётся сразу с единственным кошельком «Наличные» (`cash_name` — его имя на языке пользователя) |
| `unlock_vault` | `UnlockInput { path, password }` | `VaultInfo` |
| `lock_vault_cmd` | — | `()` |
| `change_password` | `ChangePasswordInput { current_password, new_password }` | `()`; текущий пароль проверяется расшифровкой файла (`SECRET` при ошибке), `.bak` не пишется |

### vault recents (`crates/app/src/commands/vault_recents.rs`)
| Команда | Вход | Выход |
|---|---|---|
| `recent_vaults` | — | `Vec<RecentVault>` |
| `forget_recent_vault` | `SelectVaultInput { path }` | `()` (убирает из списка, файл не трогает) |
| `pick_vault_file` | `save: bool` | `Option<String>` (путь, либо `null` если диалог закрыт); при `save` к имени без `.wallet` расширение дописывается |

### stores
| `list_stores` | — | `Vec<Store>` (плоско) |
| `get_store_tree` | — | `Vec<StoreTreeNode { store, depth, path, own_usd, own_base, subtree_usd, subtree_base, own_tx_count, can_add_child, children }>` — дерево с суммами; источник для сайдбара, страницы «Кошельки» и выбора узла |
| `get_wallet_catalog` | — | `WalletCatalog { kinds[{kind, max_roots, user_creatable, default_purpose, sub_types[{sub_type, shape, default_purpose, credit}]}], providers[{provider, supported}] }` — правила типов кошельков, без vault |
| `create_store` | `CreateStoreInput { name, kind, color, note, parent_id?, sub_type?, binding?, purpose?, liability?, contact_id?, custody?, property?, secured_by?, provider?, opening? }` | `Store`. `opening: { asset, amount }` — начальный остаток: одно пополнение в служебной категории `opening_balance` (не доход), проверяется до записи — при ошибке кошелёк не создаётся; не для имущества и кредитов. Субсчёт: одного типа с родителем, глубина 1, `sub_type` из каталога, `binding` нужной формы (рынок подставляется сам). Корень: тип `user_creatable`, `cash` — ровно один. Коды: `CASH_SINGLETON`, `WALLET_NOT_CREATABLE`, `SUB_TYPE_NOT_ALLOWED`, `BINDING_REQUIRED`, `BINDING_MISMATCH`, `WALLET_NOT_APPLICABLE` |
| `update_store` | `UpdateStoreInput { id, name, color, note, purpose?, binding?, liability?, contact_id?, custody?, property?, secured_by?, provider? }` | `Store` (место в дереве не меняет; `null` в `purpose`/`binding`/`custody`/`property`/`provider` — не менять; смена привязки отклоняется, если проведённые операции в неё не влезут) |
| `list_envelopes` | `storeId?` | `EnvelopeView[] { envelope, free, pct?, short }` |
| `create_envelope` | `EnvelopeDraft { store_id, name, asset, target?, goal_id? }` | `Envelope` — актив должен подходить к привязке кошелька |
| `update_envelope` | `UpdateEnvelopeInput { id, draft }` | `Envelope` |
| `fund_envelope` | `FundEnvelopeInput { id, amount }` | `Envelope` — `>0` откладывает из свободного остатка (`INSUFFICIENT_FUNDS`, если не хватает), `<0` возвращает |
| `move_between_envelopes` | `MoveEnvelopeInput { from, to, amount }` | — (один кошелёк и актив) |
| `delete_envelope` | `id` | — |
| `move_store` | `MoveStoreInput { id, parent_id? }` | `Store` — перенос субсчёта; `parent_id: null` = «сделать отдельным кошельком»; корень не переносится |
| `store_move_targets` | `id` | `StoreMoveTargets { parents: Vec<StoreTreeNode> (плоско), can_detach }` — куда можно перенести |
| `archive_store` | `id, archived` | `()` — каскадно на всё поддерево; восстановление под архивным родителем — ошибка |
| `delete_store` | `id` | `()` — только без операций и без субсчетов |
| `get_store_overview` | `store_id, include_children?` (по умолчанию `true`) | `StoreOverview` — см. `docs/data-model.md` |

Ошибки правил дерева приходят со своими кодами (ключи `errors.<CODE>` в локалях):
`STORE_CYCLE`, `STORE_TOO_DEEP`, `STORE_PARENT_ARCHIVED`, `STORE_NAME_TAKEN`,
`STORE_ROOT_MOVE`, `STORE_RESTORE_PARENT_FIRST`. Файл до разлома схемы Ф2 —
`INCOMPATIBLE_VAULT`.

### transactions

Одна команда на добавление и одна на правку: вход — `TransactionInput`,
объединение с внутренним тегом `type` (только IPC, в vault не попадает).

| Команда | Вход | Выход |
|---|---|---|
| `list_transactions` | `filter?: TxFilter { scope: StoreScope, kinds[], contact_id?, flow_id?, from?, to?, hide_internal, text? }` | `Vec<Transaction>` — новые сверху. `from`/`to` — `YYYY-MM-DD` включительно, по часовому поясу из настроек; `hide_internal` убирает переводы и обмены внутри одного кошелька верхнего уровня; `text` ищет по комментарию, категориям, активам, кошелькам, контактам и хешу. Без `filter` — все операции |
| `get_tx_graph` | `filter?: TxFilter, collapsed?: string[]` | `TxGraph { currency, lanes[], nodes[], edges[], columns }`. Дорожка — кошелёк в порядке дерева; узел — операция на дорожке (`column` — порядковый номер по времени, `lane` — индекс дорожки, `value` — сумма в базовой валюте на момент операции); рёбра `seq` / `transfer` / `convert` / `flow` всегда идут вперёд по времени. `collapsed` — кошельки, чьи субсчета сворачиваются в их дорожку. Больше 2000 узлов → `TOO_LARGE` |
| `add_transaction` | `TransactionInput` | `Transaction` — после автокатегоризации и проверок V1–V14 |
| `preview_transaction` | `PreviewTransactionInput { transaction, replaces? }` | `TxPreview { changes[{store_id, store_name, asset, before, after, delta_base}], base_currency, flow, warnings[], error? }` — ничего не пишет; `replaces` — id редактируемой операции, её эффект вычитается; отказ валидации приходит в `error` строкой `CODE: message` |
| `update_transaction` | `UpdateTransactionInput { id, transaction: TransactionInput }` | `Transaction` — `id` и `origin` сохраняются, холдинги пересчитываются |
| `delete_transaction` | `id` | `()` |
| `get_adjustment_draft` | `AdjustmentDraftInput { store_id, at?, replaces? }` | `AdjustmentDraft { store_id, at, lines[AdjustmentLine { asset, before, after = before, price }] }` — что счёт держит по учёту на момент `at` (пусто — сейчас), без операции `replaces`; экран «Сверка с реальностью». Сама корректировка — `add_transaction` с `TransactionInput { type: "adjustment", common, store_id, lines[{ asset, actual }] }` |
| `transaction_kinds_for_store_kind` | `kind: StoreKind` | `TransactionKinds { verbs[], special[], default }` (значения — `TxKind`) |

`TransactionInput` — `{ type: "topup" | "payment" | "transfer" | "convert" | "trade_spot" | "trade_futures" | "defi" | "asset_sell", common, …поля вида }`.
`common` — `TxCommonInput { timestamp?, comment, categories, flow_id?, contact_id?, price_usd?, fx_override? }`;
`price_usd` — цена основного актива (для двухактивных видов — получаемого).
Комиссия — `FeeInput { asset, amount }`; стороны — `AccountRef`
(`{ store }` / `{ contact: { contact_id, account_id } }` / `{ raw }`).
Фьючерс передаёт `venue` и `symbol`, контракт строится на бэкенде.

### categories и правила
| Команда | Вход | Выход |
|---|---|---|
| `list_categories` | — | `Vec<Category>` |
| `create_category` | `CreateCategoryInput { key?, name, group }` | `Category` (ключ выводится из имени и делается уникальным) |
| `update_category` | `UpdateCategoryInput { key, name?, group }` | `Category` (пустое имя — встроенная подпись) |
| `archive_category` | `key, archived` | `()` |
| `delete_category` | `key` | `()` — только неиспользуемую |
| `list_categorization_rules` | — | `Vec<CategorizationRule>` в порядке применения |
| `create_categorization_rule` | `CreateRuleInput { pattern, category }` | `CategorizationRule` (в конец списка) |
| `update_categorization_rule` | `UpdateRuleInput { id, pattern, category, active }` | `CategorizationRule` |
| `delete_categorization_rule` | `id` | `()` |
| `reorder_categorization_rules` | `ids: Vec<String>` | `()` |

### contacts
| Команда | Вход | Выход |
|---|---|---|
| `list_contacts` | — | `Vec<Contact>` |
| `create_contact` | `ContactDraft { name, kind, note, accounts }` | `Contact` (реквизиты без `id` — новые, активные) |
| `update_contact` | `UpdateContactInput { id, contact: ContactDraft }` | `Contact` (убранный реквизит с операциями становится неактивным) |
| `archive_contact` | `id, archived` | `()` |
| `delete_contact` | `id` | `()` — только если на контакт ничего не ссылается |
| `get_contact_turnover` | `ContactTurnoverInput { contact_id, account_id? }` | `ContactTurnover { rows[{ asset, sent, received }], sent_base, received_base, tx_count }` |
| `get_contact_balances` | — | `Vec<ContactBalance { contact_id, asset, receivable, payable }>` — личные долги (сумма считается из переводов с `debt`) |
| `list_debt_terms` | `contactId?` | `DebtTerms[] { id, contact_id, asset, due_at?, note }` — только срок и заметка, суммы в них нет |
| `set_debt_terms` | `DebtTermsDraft { contact_id, asset, due_at?, note }` | `DebtTerms \| null` — заменяет условия долга контакта в этом активе; пустые срок и заметка удаляют их. Просроченный и близкий срок попадает в `get_upcoming` (`kind: debt`) |

### pricing
| `search_assets` | `query` | `Vec<AssetSearchHit { symbol, name, coingecko_id?, chain?, contract?, rank }>` |
| `refresh_all` | — | `RefreshResult { prices, fx, restamped, updated_at }` + событие; `restamped` — сколько сторон операций получили цену |
| `ensure_fx` | `EnsureFxInput { currencies, date? }` | `EnsureFxOutput { points, missing }`; добирает курсы на день (UTC) из open.er-api → Frankfurter/ECB → ЦБ РФ, откатываясь до 7 дней назад; при находке — `prices-updated` |
| `open_explorer` | `url` | `()` (default browser) |
| `get_cached_price` | `AssetPriceInput { symbol, coingecko_id?, chain?, contract?, at? }` | `CachedPrice { price_usd, ts }` — курс, ближайший к `at` (пусто = сейчас); `ts` — когда снят курс, пусто — курса нет |
| `convert_amount` | `ConvertAmountInput { from, to, amount, at? }` | `ConvertedAmount { amount? }` — по курсу на `at`, кросс через USD; `null`, если курса нет |
| `get_candles` | `GetCandlesInput { symbol, interval_min, limit≤1000 }` | `GetCandlesOutput { candles: Vec<CandlePoint { ts, open, high, low, close, volume }> }` (Binance klines, **без vault**) |

### portfolio
| `get_summary` | — | `PortfolioSummary { base_currency, fx_per_usd, total_usd, total_base, assets, by_chain, by_store, assets_base, liabilities_base, net_worth_base, prices_as_of? }` (деривативы не входят; `prices_as_of` — время самой свежей цены/курса в кэше) |
| `get_net_worth` | — | `NetWorth { base_currency, financial, non_financial, receivables, liabilities, payables, net }` |
| `set_manual_price` | `SetManualPriceInput { asset_key, unit_price, currency }` | `ManualPrice` — оценка имущества в узле `asset`, на всю историю |
| `clear_manual_price` | `asset_key` | `()` — назад к цене покупки |
| `get_history` | `base_currency?` | `Vec<HistoryPoint { ts, ts_ms, value_usd, value_base }>` |
| `get_realized_pnl` | — | `RealizedPnl` |
| `get_store_history` | `store_id, include_children?, base_currency?` | `Vec<HistoryPoint>` — по узлу и (по умолчанию) его субсчетам |
| `get_store_pnl` | `store_id, include_children?` | `RealizedPnl { total_base, fees_base, legs[] }` — по той же области; комиссии отдельно |

### calc
| `calc_preview` | `CalcInput { mode, amount, price_in, price_out, target_price?, stop_price?, fee_pct? }` | `CalcResult { gross_in, fee, spent, gross_out, net_out, exit_value, pnl, roi_pct, risk, risk_pct }` |

### app settings (вне vault, в конфигах приложения)
| `get_app_settings` | — | `AppSettings { theme, locale, base_currency, favorite_currencies[] }`. Не падает: битый/отсутствующий файл → дефолты |
| `update_app_settings` | `UpdateAppSettingsInput { theme?, locale?, base_currency?, favorite_currencies?, timezone?, inflation?, enrichment_opt_in? }` | `AppSettings` после нормализации. Смена `base_currency` пересчитывает холдинги открытого vault. Коды приводятся к верхнему регистру, невалидные (не 3 буквы) отбрасываются. `timezone` — IANA-пояс, по которому режутся дни и месяцы (§7.6); пустая строка — пояс ОС, неизвестное имя отбрасывается. `inflation` — вся таблица «валюта → % в год» целиком (по умолчанию rub 8, usd 3, eur 2.5, thb 1) |
| `set_quick_add_shortcut` | `shortcut?: string` (`CmdOrCtrl+Shift+Space`; пусто/`null` — выключить) | `AppSettings`. Сначала регистрирует глобальное сочетание, сохраняет только при успехе; не удалось (занято, неверный формат, нет глобальных клавиш) → `SHORTCUT_FAILED`, прежнее сочетание восстанавливается. Срабатывание показывает окно и шлёт событие `quick-add` |
| `get_supported_currencies` | — | `CurrencyCatalogOutput { catalog: { fiat[], crypto[] }, notice? }`. `notice` ставится, только если ни один провайдер не ответил: `cached` — взят последний успешный список, `builtin` — встроенный (сеть ещё ни разу не отвечала); `null` — провайдеры ответили сейчас, даже если список не изменился |

Файлы: `app_settings.json` и `currencies.json` в `app_config_dir`, обычный
JSON без шифрования — там нет ничего секретного. Логика типов, нормализации и
лестницы деградации — в `crates/core/src/appconfig.rs` (юнит-тесты там же).

### losses, chains, rates (`crates/app/src/commands/losses.rs`)

Потеря операции = комиссия по штампу + разница между отданным и полученным по
**рыночному** курсу на момент операции (не по введённому вручную курсу), только
для обмена, спот-сделки и продажи имущества. Сводка потерь — `get_report` с
`kind: losses`.

| Команда | Вход | Выход |
|---|---|---|
| `get_tx_loss` | `id` | `TxLoss { tx_id, currency, fee, slippage, total, internal, complete }` в базовой валюте |
| `get_tx_losses` | — | `Record<id, TxLoss>` — все операции с ненулевой или неполной потерей (колонка списка) |
| `list_flows` | `from?`, `to?` (RFC3339) | `Vec<FlowSummary { flow_id, currency, links[{ tx, loss }], started_at, ended_at, gave[], got[], gave_value, got_value, loss, difference }>` — новые первыми |
| `link_transactions` | `LinkTransactionsInput { ids, flow_id? }` | `String` — id цепочки: заданный, иначе цепочка одной из операций, иначе новый |
| `unlink_transaction` | `id` | `()` |
| `get_fx_table` | — | `FxTable { base, rows[{ currency, per_base, in_base, updated_at, tracked }] }` — последние курсы к базовой |

### enrichment (`crates/app/src/commands/enrichment.rs`)

Только при `AppSettings.enrichment_opt_in == true`: после `add_transaction` /
`update_transaction` с хешем команда в фоне вызывает `TxInfoService::inspect`
(мьютекс отпущен на время сети), пишет `EnrichmentRecord { Pending }` и шлёт
`enrichment-ready`. Ничего не применяется само.

| Команда | Вход | Выход |
|---|---|---|
| `get_enrichments` | `tx_id?` | `Vec<EnrichmentRecord>` — новые первыми |
| `apply_enrichment` | `id` | `EnrichmentRecord` (`Applied`); комиссия дописывается в операцию, если её не было, через `update_transaction` с проверками |
| `reject_enrichment` | `id` | `EnrichmentRecord` (`Rejected`); операция не меняется |
| `re_enrich` | `tx_id` | `Option<EnrichmentRecord>` — проверить ещё раз сейчас; без согласия — `INVALID_INPUT` |

### analytics (`crates/app/src/commands/analytics.rs`)

Отчёты считаются в core (`service/reports.rs`). Суммы — в валюте отчёта
(`filter.currency`, по умолчанию валюта бюджета) по курсу на дату операции;
дни и месяцы — в поясе `AppSettings.timezone`. `real_terms` пересчитывает каждый
месяц в сегодняшние деньги по `AppSettings.inflation` (справочно).

| Команда | Вход | Выход |
|---|---|---|
| `get_report` | `kind: ReportKind` (`cashflow` \| `year` \| `categories` \| `networth` \| `stores` \| `losses` \| `pnl` \| `portfolio` \| `inflation`), `filter: ReportFilter { scope, from?, to? (YYYY-MM, по умолчанию последние 12 месяцев), year?, currency?, real_terms }` | `Report` (в TS — `AnalyticsReport`, чтобы не путать с DOM-типом `Report`): внешне помеченный вариант с отчётом своего вида и `meta { currency, months, real_terms, inflation_rate? }` |
| `export_report_csv` | те же `kind`, `filter` | `Option<String>` — путь файла; `None`, если диалог закрыли. CSV: UTF-8 с BOM, `;`, CRLF, колонка валюты в каждой строке |
| `get_calendar` | `month?` (`YYYY-MM`, по умолчанию текущий) | `CalendarMonth { month, currency, allowance?, days[{ date, income, spent, daily, tx_count, status }], upcoming[] }`; `status`: `none` \| `under` \| `near` (> 80 % дневного бюджета) \| `over` \| `future` |

### planning (`crates/app/src/commands/planning.rs`)

Дни, недели и месяцы режутся в поясе `AppSettings.timezone` (иначе — пояс ОС,
иначе UTC); «сейчас» берёт команда. Суммы бюджета — в валюте бюджета
(`Settings.budget_currency`, пусто — базовая) по курсу на дату операции.

| Команда | Вход | Выход |
|---|---|---|
| `list_automations` | — | `Vec<Automation>` по имени |
| `create_automation` | `AutomationInput { name, active, preset?, schedule, starts_at, template: TransactionInput }` (пресеты по типу кошелька — в `get_wallet_catalog`) | `Automation`. Шаблон — один из четырёх глаголов и проходит те же проверки, что операция |
| `update_automation` | `UpdateAutomationInput { id, automation }` | `Automation`; обработанные даты остаются обработанными |
| `delete_automation` | `id` | `()` — вместе с историей запусков; созданные операции остаются |
| `run_automation` | `id` | `Transaction` «сейчас»; не хватает средств — `INSUFFICIENT_FUNDS` с суммами |
| `list_automation_runs` | `automation_id?` | `Vec<AutomationRun>`, новые первыми |
| `retry_automation_run` | `run_id` | `Transaction`: баланс проверяется на текущий момент, дата операции — сейчас |
| `skip_automation_run` | `run_id` | `AutomationRun` с `Skipped` |
| `list_budgets` / `create_budget` / `update_budget` / `delete_budget` | `BudgetInput { active, target, period, limit, currency }` / `UpdateBudgetInput { id, budget }` / `id` | `Budget` |
| `get_budget_status` | — | `Vec<BudgetStatus { budget, spent, remaining, used, period_start, period_end }>` — активные лимиты, траты групп «обязательные» и «ежедневные» в валюте лимита |
| `get_daily_budget` | — | `DailyBudget { mode, currency, date, per_day, carry, available, spent_today, left_today, income, fixed, savings_plan, free, days, next_income?, manual_limit? }` |
| `get_month_summary` | `month?` (`YYYY-MM`, по умолчанию текущий) | `MonthSummary { income, fixed, daily, savings_plan, savings_fact, deviation, saved_pct, savings_moved, budget_in, budget_out, not_executed, complete }` |
| `list_goals` / `create_goal` / `update_goal` / `delete_goal` | `GoalInput { name, active, target_amount, currency, starts_at, deadline?, priority, monthly_commitment?, scope }` / `UpdateGoalInput { id, goal }` / `id` | `Goal`; список — по важности, затем по дедлайну |
| `get_goal_progress` | — | `Vec<GoalProgress { goal, saved, contributed, fx_gain, pct, months_total?, months_left?, need_per_month?, realistic, issues, forecast?, accumulating }>` |
| `get_goal_calendar` | `year` | `GoalCalendar { year, currency, months[{ month, need, saved, shortfall, done }] }` |
| `list_reminders` / `create_reminder` / `update_reminder` / `delete_reminder` | `ReminderInput { text, due_at }` / `UpdateReminderInput { id, reminder }` / `id` | `Reminder` |
| `set_reminder_done` | `id, done` | `Reminder` |
| `list_price_alerts` / `create_price_alert` / `update_price_alert` / `delete_price_alert` | `PriceAlertInput { asset: AssetIdent, condition, price, currency, active, store_id? }` / `UpdatePriceAlertInput { id, alert }` / `id` | `PriceAlert`; правка взводит алерт заново. `store_id` — биржа, из терминала которой заведён алерт (только корневой кошелёк `exchange`, иначе `INVALID_INPUT`) |
| `list_utility_items` / `create_utility_item` / `update_utility_item` / `delete_utility_item` | `UtilityInput { name, active, kind: metered{ unit, tariff } \| static{ amount } \| housing{ amount, deposit?, due_day, automation_id? }, currency, property_id, store_id?, category }` / `UpdateUtilityInput { id, item }` / `id` | `UtilityItem` — ЖКХ всегда принадлежит имуществу (`property_id` — корневой кошелёк `property`, иначе `MISSING`); удаление уносит показания, платежи остаются |
| `add_meter_reading` | `MeterReadingInput { item_id, value, period (YYYY-MM) }` | `MeterReading`; показание за тот же месяц заменяется; меньше прошлого или больше следующего — `INVALID_INPUT` |
| `delete_meter_reading` | `id` | `()` |
| `get_property_summary` | `storeId` | `PropertySummary { value_base, debt_base, equity_base, utilities_month_base, loan_month_base, monthly_cost_base, utility_count }` — долг по кредитам с `secured_by` на объект |
| `get_meter_trend` | `item_id` | `UtilityTrend { months[{ period, reading?, consumption?, amount }], average?, delta_pct?, forecast?, anomaly, monthly }` — среднее за 6 месяцев со счётом, прогноз — линейная регрессия, аномалия — > 1.5× среднего прежних |
| `get_utility_payment_draft` | `item_id`, `period?` (по умолчанию текущий месяц) | `UtilityPaymentDraft { transaction (payment), consumption?, includes_deposit }` — форма «Оплатить» открывается с ним; комментарий = название счёта, категория счёта |
| `update_meter_reading` | `UpdateMeterReadingInput { id, value, period }` | `MeterReading` — исправить показание; те же правила монотонности, id сохраняется, при отказе старое значение остаётся |
| `get_utility_bill` | `property_id`, `period?` | `UtilityBill { pay_day?, due_at, lines[{ item, previous_reading?, reading?, consumption?, amount?, paid, includes_deposit }] }` — счёт ЖКХ объекта за месяц; `amount = null` у счётчика без показания |
| `pay_utility_bill` | `PayUtilityBillInput { property_id, period, store_id, timestamp?, lines[{ item_id, amount, reading? }] }` | `Vec<Transaction>` — показания + по платежу на строку с `meta.utility_item_id`; всё или ничего |
| `set_deposit_terms` | `SetDepositTermsInput { store_id, terms? }` | `Store` — ставки (история), капитализация, срок, автоначисление, ПДС; только сабсчета `savings`/`deposit`/`pds` банка |
| `get_deposit_report` | `store_id` | `DepositReport { balance, rate?, accrued_month, accrued_year, posted_year, projected_year, matures_at?, at_maturity?, pds? }` — проценты по дням по фактическому балансу |
| `get_interest_tax` | `year?` | `InterestTaxReport { max_key_rate?, limit?, income, taxable, tax, remaining_free? }` — НДФЛ 13% сверх 1 000 000 ₽ × макс. ключевая ставка на 1-е числа года |
| `refresh_key_rates` | — | `u32` — ключевая ставка ЦБ РФ (SOAP `DailyInfo.KeyRate`) с начала прошлого года в `VaultDoc.key_rates` |
| `set_loyalty_info` | `SetLoyaltyInfoInput { store_id, info: LoyaltyInfo }` | `Store` — программа бонусов; цена балла становится ручной ценой актива баллов |
| `adjust_points` | `AdjustPointsInput { store_id, delta, comment, source_store_id?, timestamp? }` | `Transaction` — начисление (topup, категория `cashback`) или списание баллов |
| `get_cashback_report` | `period?` (`month` \| `year`) | `CashbackReport { cashback_base, points_base, by_source[], programmes[] }` |
| `get_upcoming` | `days?` (30, не больше 366) | `Vec<UpcomingItem { kind, source_id, at, date, title, store_id?, amount?, asset?, shortfall? }>` — автоматизации с прогнозом «не хватит N», аренда, платежи по кредитам, напоминания и отклонённые запуски |

### settings (vault)
| `get_settings` | — | `Settings` — только vault-scoped поля |
| `update_settings` | `UpdateSettingsInput` | `Settings` (`savings_rate` 0..1; `daily_budget_mode`; `budget_currency`, пусто — базовая). Логика — в core (`UnlockedVault::update_settings`) |
| `dev_dump_vault` | — | `DevDumpVaultOutput { json }` — расшифрованный `VaultDoc` одним pretty-JSON (только DEV-экран) |
| `dev_export_vault` | — | `Option<String>` — путь записанного файла, `None` если пользователь закрыл диалог. Нативный save-диалог + запись делает Rust: webview не умеет скачивать файлы (`URL.createObjectURL` + `<a download>` в WebKitGTK не работает) |
| `vault_schema` | — | `serde_json::Value` — JSON Schema `VaultDoc` из `schemars` (для подсказок в редакторе; форма документа живёт только в Rust) |
| `dev_apply_vault_json` | `json` | `String` — каноническая пересериализация записанного документа. Отказ (до записи, `.bak` не трогается) на невалидном JSON, неизвестной схеме или неверной версии payload |
| `dev_open_vault_json_in_editor` | — | `String` — путь scratch-файла. Пишет документ и запускает `$EDITOR` detached, не дожидаясь |
| `dev_reload_vault_json_from_editor` | — | `String` — содержимое scratch-файла. Путь не принимается: фронт может читать только этот файл |

### exchange (только чтение)
| `inspect_tx` | `InspectTxInput { tx_hash, network? }` | `InspectTxResult { fetched, info?, hints[], notice? }` |
| `bybit_executions` | `BybitExecutionsInput { category, symbol?, limit?, store_id? }` | `BybitExecutionsResult { executions[], notice? }` |
| `exchange_key_status` | `ExchangeKeyStatusInput { store_id }` | `ExchangeKeyStatus { configured, hint?, spot_store_id?, futures_store_id? }` |
| `set_exchange_key` | `SetExchangeKeyInput { store_id, api_key, api_secret }` | `ExchangeKeyStatus` (субсчета прежнего ключа сохраняются) |
| `set_import_interval` | `SetImportIntervalInput { store_id, interval_sec }` | — интервал фонового импорта конкретной биржи (`0` — выкл, иначе 60..86400) |
| `get_terminal_state` | `storeId` | `TerminalState { watchlist, active_symbol?, timeframe, indicators, drawings{symbol→json}, layout }` — терминал хранится в самой бирже; без выбранной биржи `PROVIDER_REQUIRED` |
| `update_terminal_state` | `UpdateTerminalStateInput { store_id, patch: TerminalPatch }` | `TerminalState` — меняются только переданные поля |
| `save_terminal_drawings` | `SaveTerminalDrawingsInput { store_id, symbol, json }` | — пустой экспорт удаляет рисунки символа; больше 256 КБ — `TOO_LARGE` |
| `set_exchange_targets` | `SetExchangeTargetsInput { store_id, spot_store_id?, futures_store_id? }` | `ExchangeKeyStatus` — куда записывать спот и фьючерсы; только биржевые субсчета этого кошелька |
| `clear_exchange_key` | `ClearExchangeKeyInput { store_id }` | `ExchangeKeyStatus` |

### Bybit import — staging (D2.4)
| `import_bybit_history` | `ImportBybitHistoryInput { store_id, category?, limit? }` | `ImportBybitHistoryOutput { staged, notice? }` — скачивает сделки и **кладёт их в staging**, операции не создаёт |
| `list_pending_imports` | — | `ListPendingImportsOutput { imports: Vec<PendingImport> }` (новые сверху) |
| `confirm_pending_imports` | `ConfirmPendingImportsInput { exec_ids }` (пусто = все) | `ConfirmPendingImportsOutput { confirmed }` — `kind = fill`: спот → `TradeSpot`, linear/inverse → `TradeFutures` (комиссия в валюте расчёта), на субсчёт из ключа, иначе Unified; `transfer` → `Transfer` между субсчетами Funding/Unified/Earn; `earn_income` → `Topup` (`deposit_interest`) на Earn; не прошедшая проверку остаётся в staging |
| `discard_pending_imports` | `DiscardPendingImportsInput { exec_ids }` (пусто = все) | `DiscardPendingImportsOutput { discarded }` |
| `create_bybit_accounts` | `CreateBybitAccountsInput { store_id, funding_name, unified_name, earn_name }` | `Vec<String>` — субсчета Funding / Unified / Earn (недостающие создаются) |
| `bybit_balances` | `store_id` | `Vec<ExchangeAccountBalance { account, store_id?, rows[{ coin, exchange, ledger, diff }] }>` — балансы FUND/UNIFIED/EARN по API рядом с учётом субсчёта |

Инварианты блока:
- **Ни одна команда не создаёт операции сама.** Только `confirm_pending_imports`;
  фоновый опрос (`ExchangeProfile.import_interval_sec` у каждой биржи, `0` = выкл) кладёт сделки в
  `pending_imports` и шлёт событие — пользователь решает сам.
- Импорт берёт сделки, внутренние переводы (`/v5/asset/transfer/query-inter-transfer-list`) и доход Earn (`/v5/earn/yield`); категория сделки ставится по запросу, а не угадывается по символу.
- Ключ ищется у кошелька и его родителей; чужой ключ не подставляется.
- Дедупликация по `execId` (`PendingImport.exec_id`; у переводов `transfer:<id>`, у Earn `earn:<id>`), поэтому повторный импорт
  и перезапуски фонового опроса не создают дублей.
- Мусорные сделки (нет `execId`, нечитаемые количество/цена, нулевое
  количество) отбрасываются на границе и в staging не попадают.
- `notice` — код причины: `no_key` | `unreachable` | `rate_limited` |
  `forbidden_region`. Ошибок на сетевых сбоях этот блок не бросает.

Команды этого блока **не бросают ошибок** для сетевых сбоев: вместо этого
возвращают пустой список и `notice`. Форма сделки остаётся ровно такой, какой
её оставил пользователь.

- `notice` для `inspect_tx`: `empty` | `unavailable` | `choose_network`.
- `notice` для `bybit_executions`: `no_key` | `unreachable` | `rate_limited` |
  `forbidden_region`.
- `store_id` в `bybit_executions` опционален: если не указан — подъём по предкам
  к узлу-бирже (Phase N).
- Секрет API никогда не возвращается наружу: `ExchangeKeyStatus` содержит только
  маску вида `abcd…wxyz`.

## Доменные типы в IPC (важно для UI)

- `Chain` — union строк; `Chain`/`ChainInfo` метаданные (CoinGecko id, explorer
  patterns) не передаются по IPC, а живут во фронтенд-константах (см. модель).
- `Amount` — в TS это просто `string` (все денежные поля).
- `StoreKind` — `"exchange" | "hot_wallet" | "cold_wallet" | "bank" | "cash" | "asset" | "liability"`.
- `Transaction` — union одно-ключевых объектов, ключ = вид операции
  (`{ topup: TopupTx } | { transfer: TransferTx } | …`). На фронте сужение
  через `body(tx, 'topup')` из `src/shared/lib/transactions.ts`; `txLegs(tx)`
  разворачивает запись в подписанные ноги с ролями.
- `TxMeta.categories` — ключи категорий; бэкенд нормализует их (trim,
  lowercase, дедуп) и проверяет, что категория есть, не в архиве и подходит
  по группе. Новых категорий операция не создаёт.
- Биржевая категория Bybit (`spot`/`linear`/`inverse`) в TS называется
  `BybitCategory`, чтобы не путаться с `Category`.

## Добавление новой команды (чеклист)

1. Тип входных данных в `ipc_contract.rs` (`#[derive(Serialize, Deserialize, TS)]`).
2. Метод `UnlockedVault` в `service.rs` + юнит-тест.
3. Хендлер в `crates/app/src/commands/<модуль>.rs` + регистрация в
   `generate_handler!` (`lib.rs`).
4. `mise run gen-types` → новый тип в `src/shared/api/ipc.gen.d.ts`.
5. Если команда async и держит vault — скопировать данные до `.await` (правило №3 из AGENTS.md).