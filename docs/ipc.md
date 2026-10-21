# IPC-контракт (frontend ↔ backend)

Все команды — `#[tauri::command]` в `crates/app/src/commands/`. Типы — из
`crates/core/src/ipc_contract.rs` и домена core, генерируются ts-rs в
`src/shared/api/ipc.gen.d.ts` через `mise run gen-types`.

Ошибки: команды возвращают `Result<T, String>`; строка —
`ERR_CODE: message`, где `ERR_CODE` — метод `Error::code()`
(например `CRYPTO`, `MISSING`, `INVALID_INPUT`, `REQUIRES_UNLOCK`, `NOT_FOUND`,
`SYNC_CONFLICT`).
Frontend парсит код и показывает локализованное сообщение.

## События

| Событие | Payload | Когда |
|---|---|---|
| `prices-updated` | `RefreshResult` | после `refresh_all` (кнопка, трей, таймер «Интервал обновления цен» в планировщике, пока окно на экране) и `ensure_fx` (если что-то найдено); экраны обновляют «ценовые» ключи из `use-cache-events`, а не из ответа команды |
| `rates-updated` | — | после `refresh_rates` (кнопка «Обновить» в настройках инфляции и суточный `rates_tick` планировщика) и `set_inflation_mode`; фронт обновляет `inflation-overview`, `report`, `interest-tax` (`use-cache-events`) |
| `sources-updated` | — | после `datasource_*` и фонового `sources_tick`; фронт обновляет `data-sources` (вместе с ним шлются `prices-updated` и `rates-updated`) |
| `bybit-import-pending` | `{ staged: number }` | фоновый опрос Bybit нашёл новые сделки (D2.4) |
| `automation-created` | `{ tx_ids: string[] }` | планировщик создал операции автоматизаций |
| `automation-rejected` | `AutomationRejectedEvent { run_id, automation_id, name, planned_at, reason }` | запуск не создан: не хватило средств, лимит кредитки или операция не прошла проверку (D26); тост с «Повторить / Пропустить» + системное уведомление |
| `automation-skipped` | `{ count }` | после долгого перерыва старые даты сверх 24 записаны как пропущенные |
| — (уведомление) | ключ `automation-estimated:<tx_id>` | автоматизация с разной суммой записала операцию примерно: «уточните сумму» |
| `workflows-ran` | `{ runs, queued }` | планировщик запустил схемы (узлы); `queued` — операций ушло в очередь подтверждения. Созданные операции приходят ещё и через `automation-created` |
| `price-alert` | `{ alert_id }` | сработал ценовой алерт (один раз) |
| `reminder-due` | `{}` | наступило время напоминаний — обновить их список |
| `reminders-changed` | `{}` | синхронизация создала или сдвинула авто-напоминания |
| `notification` | `NoticeRecord { key, category, title, body, link?, at, delivered_at, read }` | показано уведомление (напоминание, алерт, нехватка денег, бюджет, автоматизация, импорт Bybit): колокольчик и тост. `link` — имя маршрута или `wallet:<id>`. Каждый `key` показывается один раз за всё время — журнал в vault |
| `enrichment-ready` | `{ tx_id }` | обозреватель вернул данные по хешу операции; находка ждёт «Применить / Отклонить» |
| `vault-mirror-failed` | `string` (причина) | копия vault во внешний файл не записалась (`platform_fs::mirror_sink`); vault цел, статус — `vault_link_status` |
| `sync-status` | `SyncStatus` | изменилось состояние синхронизации устройств: роль, соединения, фазы, прогресс (`docs/sync.md`) |
| `sync-offer` | `SyncOfferEvent { session_id, peer_name }` | рядом устройство с этим же vault, ещё не сопряжённое: баннер «Сопрячь» |
| `sync-pairing` | `SyncPairingEvent { session_id, peer_name, code }` | показать 6-значный код; на втором устройстве он тот же |
| `sync-plan-ready` | `SyncPlanEvent { session_id, peer_name, reason, summary }` | список изменений ждёт решения (`sync_decide_plan`): первая пачка при «Спрашивать», первое сопряжение или «временно ненадёжное» устройство |
| `vault-synced` | `VaultSyncedEvent { session_id, batch: AppliedBatch }` | пачка с другого устройства применена; фронт перечитывает все запросы, уведомление с «Отменить» (`sync_undo`) |
| `sync-clock` | `SyncClockEvent { session_id, peer_name, offset_ms, unreliable }` | часы устройств расходятся (5 мин – 24 ч) или прыгнули: устройство «временно ненадёжное» до конца сессии |
| `sync-error` | `SyncErrorEvent { code, message, peer_name? }` | сессия не началась или прервалась: `SYNC_OTHER_VAULT`, `SYNC_VERSION_MISMATCH`, `SYNC_CLOCK_SKEW`, `SYNC_REVOKED`, `SYNC_REJECTED`, `SYNC_CHANNEL` |
| `transfer-receive` | `TransferReceiveView` | шаг приёма хранилища: ожидание, запрос, код, прогресс, готово (`saved_path`), ошибка (`SYNC_TRANSFER_EXPIRED`, `SYNC_BAD_CODE`, `SYNC_TRANSFER_DECLINED`, `SYNC_NOT_A_VAULT`) |
| `transfer-send` | `TransferSendView` | шаг передачи: ждём «Принять», код (`wrong_code`, `attempts_left`), прогресс, готово, ошибка |
| `transfer-nearby` | `TransferNearby[]` | устройства рядом, ждущие приёма |
| `vault-changed` | `VaultStatus` | vault заблокирован без запроса окна (Android: блокировка в фоне, кнопка на уведомлении синхронизации); окно уходит на экран разблокировки |
| `vault-save-failed` | `string` или `{ reason, exiting }` | пакетная запись vault не удалась (повторяется сама); `exiting` — при выходе, окно предлагает «Повторить» / «Выйти без сохранения» |
| `vault-saved` | — | запись снова удалась после ошибки |

Фоновые задачи живут в `crates/app/src/scheduler.rs`: один поток, тик раз в 30 с,
только при разблокированном vault — импорт Bybit, автоматизации, алерты (цены их
активов — не чаще раза в 5 минут), напоминания. Системные уведомления —
`tauri-plugin-notification` (capability `notification:default`).

## Команды

### auth (`crates/app/src/commands/auth.rs`)
| Команда | Вход | Выход |
|---|---|---|
| `vault_status` | — | `VaultStatus { exists, unlocked, path, info, storage }`; `storage`: `files` (десктоп) или `app` (Android/iOS, vault в памяти приложения) |
| `create_vault` | `CreateVaultInput { path, password, cash_name? }` | `VaultInfo` — vault создаётся сразу с единственным кошельком «Наличные» (`cash_name` — его имя на языке пользователя). `path` без разделителей — имя: файл ляжет в каталог приложения; URI — `INVALID_INPUT` |
| `unlock_vault` | `UnlockInput { path, password }` | `VaultInfo`; `content://` в `path` сначала копируется в каталог приложения; у привязанного vault сначала сверяется копия — `SYNC_CONFLICT`, если изменились обе стороны |
| `lock_vault_cmd` | — | `()` |
| `flush_vault` | — | `()` — записать отложенные изменения сейчас (окно ушло в фон, «Повторить» после ошибки) |
| `exit_without_saving` | — | — выйти, хотя последние изменения записать не удалось |
| `change_password` | `ChangePasswordInput { current_password, new_password }` | `()`; текущий пароль проверяется расшифровкой файла (`SECRET` при ошибке), `.bak` не пишется |

### vault recents (`crates/app/src/commands/vault_recents.rs`)
| Команда | Вход | Выход |
|---|---|---|
| `recent_vaults` | — | `Vec<RecentVault { path, last_opened, exists, display_name, linked }>`; в режиме `app` дополняется всеми `*.wallet` каталога приложения |
| `forget_recent_vault` | `SelectVaultInput { path }` | `()` (убирает из списка, файл не трогает) |
| `pick_vault_file` | `save: bool` | `Option<PickedVault { path, display_name, imported }>` (`null`, если диалог закрыт); при `save` к имени без `.wallet` расширение дописывается. На Android/iOS открытие копирует файл в каталог приложения (`imported`), `save` — `UNSUPPORTED` (создание по имени) |

### копия во внешнем файле (`crates/app/src/commands/vault_link.rs`)

Устройство — `docs/storage-platforms.md`.

| Команда | Вход | Выход |
|---|---|---|
| `vault_link_status` | — | `VaultLinkStatus { linked, display_name, last_synced_at, last_error }` открытого vault |
| `link_vault_external` | `LinkVaultInput { existing }` | `Option<VaultLinkStatus>` (`null` — диалог закрыт); копия пишется сразу; `existing` — выбранный файл заменяется vault |
| `unlink_vault_external` | — | `()`; файл-копия остаётся на месте, право доступа (Android) отдаётся |
| `resolve_vault_conflict` | `ResolveVaultConflictInput { path, keep: local \| remote }` | `()`; `remote`: локальный vault уходит в `.bak`, копия становится vault. Потом снова `unlock_vault` |

### синхронизация устройств (`crates/app/src/commands/sync.rs`)

Устройство — `docs/sync.md`. Работает, пока vault разблокирован и синхронизация включена.

| Команда | Вход | Выход |
|---|---|---|
| `sync_status` | — | `SyncStatus { enabled, role, device_id, device_name, confirm_first, prefer_main, key_storage, sessions, has_backup, background_supported, background_lock_min, network }`; `network` — диагностика: порт, адрес, сколько устройств видно, `inbound_blocked`, `firewall_fix` (`none`/`auto`/`manual`), команды, код последней ошибки |
| `sync_set_settings` | `SyncSettingsInput { enabled?, confirm_first?, prefer_main?, device_name?, background_lock_min? }` | `SyncStatus`; включение и выключение действуют сразу; на Android включение спрашивает разрешение на уведомления |
| `sync_open_firewall` | — | `SyncStatus`; правило для портов 53318–53319/tcp из локальной подсети (pkexec или UAC); `SYNC_FIREWALL` — отменено или фаервол не найден |
| `transfer_receive_start` | — | `TransferReceiveView`; устройство видно рядом по имени 5 минут, без открытого vault |
| `transfer_receive_stop` | — | `()` |
| `transfer_receive_decide` | `accept` | `()`; «Принять» / «Отклонить» запрос на экране |
| `transfer_browse_start` | — | `TransferNearby[]`; дальше — событие `transfer-nearby` |
| `transfer_browse_stop` | — | `()` |
| `transfer_send` | `TransferSendInput { id, password }` | `TransferSendView`; `SECRET` — пароль не этого vault |
| `transfer_send_code` | `code` | `()`; код с экрана получателя (6 цифр); `SYNC_BAD_CODE` — не шесть цифр |
| `transfer_send_cancel` | — | `()` |
| `sync_pair` | `sessionId` | `()`; «Сопрячь» на баннере: код появляется на обоих устройствах |
| `sync_dismiss_offer` | `sessionId` | `()`; «Не сейчас»: сессия закрывается тихо, устройство не предлагается до перезапуска синхронизации |
| `sync_confirm_pairing` | `SyncDecisionInput { session_id, accept }` | `()`; коды совпали (`accept`) или нет |
| `sync_decide_plan` | `SyncDecisionInput { session_id, accept }` | `()`; применить список изменений или отказаться (сессия закрывается) |
| `sync_peers` | — | `Vec<SyncPeerView { device_id, name, paired_at, last_synced_at, online }>` — доверенные устройства |
| `sync_forget_peer` | `deviceId` | `()`; устройство отклоняется здесь и, по мере синхронизации, везде |
| `sync_history` | — | `Vec<AppliedBatch { id, peer_name, applied_at, summary, undone_at }>`, новые первыми (до 20 пачек, 7 дней) |
| `sync_undo` | `batchId` | `UndoReport { restored, skipped }`; откат — новая правка, уходит и на другие устройства |
| `sync_restore_backup` | — | `()`; vault как до первой пачки последней сессии (`<vault>.presync.bak`) |

### stores
| `list_stores` | — | `Vec<Store>` (плоско) |
| `get_store_tree` | — | `Vec<StoreTreeNode { store, depth, path, own_usd, own_base, subtree_usd, subtree_base, own_tx_count, can_add_child, children, top_assets, last_tx_at }>` (`top_assets` — до трёх крупнейших активов, `last_tx_at` — последняя операция с субсчетами) — дерево с суммами; источник для сайдбара, страницы «Кошельки» и выбора узла |
| `get_wallet_catalog` | — | `WalletCatalog { kinds[{kind, max_roots, user_creatable, default_purpose, sub_types[{sub_type, shape, default_purpose, credit}]}], providers[{provider, supported}] }` — правила типов кошельков, без vault |
| `create_store` | `CreateStoreInput { name, kind, color, note, parent_id?, sub_type?, binding?, purpose?, liability?, contact_id?, custody?, property?, secured_by?, provider?, opening?, card? }` | `Store`. `opening: { asset, amount }` — начальный остаток: одно пополнение в служебной категории `opening_balance` (не доход), проверяется до записи — при ошибке кошелёк не создаётся; не для имущества и кредитов. Субсчёт: одного типа с родителем, глубина 1, `sub_type` из каталога, `binding` нужной формы (рынок подставляется сам). Корень: тип `user_creatable`, `cash` — ровно один. Коды: `CASH_SINGLETON`, `WALLET_NOT_CREATABLE`, `SUB_TYPE_NOT_ALLOWED`, `BINDING_REQUIRED`, `BINDING_MISMATCH`, `WALLET_NOT_APPLICABLE`  Банк верхнего уровня с `card: FirstCard { name, currency }` создаётся вместе с первой картой (`card`, привязка к валюте), `opening` ложится на карту; всё или ничего. |
| `update_store` | `UpdateStoreInput { id, name, color, note, purpose?, binding?, liability?, contact_id?, custody?, property?, secured_by?, provider? }` | `Store` (место в дереве не меняет; `null` в `purpose`/`binding`/`custody`/`property`/`provider` — не менять; смена привязки отклоняется, если проведённые операции в неё не влезут) |
| `vehicle_update_mileage` | `VehicleMileageInput { store_id, km }` | `Store` — пробег машины на сегодня (`car.mileage_km`, `mileage_at`); не машина — `INVALID_INPUT` |
| `property_set_revalue` | `store_id, rule: RevalueRule \| null` | `()` — автооценка объекта: `pct_per_year` (`null` = инфляция валюты), `floor` обязательна при отрицательном %; включение без оценок ставит оценку сегодня по текущей цене |
| `vehicle_service_done` | `VehicleServiceDoneInput { store_id, date, km? }` | `Store` — ТО пройдено: пробег (если дан), следующее ТО = дата + `service_interval_months` и пробег + `service_interval_km` |
| `list_envelopes` | `storeId?` | `EnvelopeView[] { envelope, free, pct?, short }` |
| `create_envelope` | `EnvelopeDraft { store_id, name, asset, target?, goal_id? }` | `Envelope` — актив должен подходить к привязке кошелька |
| `update_envelope` | `UpdateEnvelopeInput { id, draft }` | `Envelope` |
| `fund_envelope` | `FundEnvelopeInput { id, amount }` | `Envelope` — `>0` откладывает из свободного остатка (`INSUFFICIENT_FUNDS`, если не хватает), `<0` возвращает |
| `move_between_envelopes` | `MoveEnvelopeInput { from, to, amount }` | — (один кошелёк и актив) |
| `delete_envelope` | `id` | — |
| `move_store` | `MoveStoreInput { id, parent_id? }` | `Store` — перенос субсчёта; `parent_id: null` = «сделать отдельным кошельком»; корень не переносится |
| `store_move_targets` | `id` | `StoreMoveTargets { parents: Vec<StoreTreeNode> (плоско), can_detach }` — куда можно перенести |
| `plan_store_change` | `StoreChangeInput { store_id, change: change_kind { kind } \| move_to { parent_id } \| "detach" \| merge_into { target_id } }` | `StoreChangePlan { store_id, kept[], archived[], conflicts[{ tx_id, timestamp, comment, message }], moved_transactions, error? }` — «Опасная зона → Сменить тип или перенести», ничего не пишет. Смена вида — только между `bank`/`crypto`/`exchange` и только у кошелька; субсчёт, который новый вид не держит, отделяется в архивный кошелёк прежнего вида; перенос — в кошелёк любого вида с подбором типа субсчёта (карта ↔ токен-счёт); слияние переписывает все ссылки на счёт и удаляет его |
| `apply_store_change` | `StoreChangeInput` | `StoreChangePlan` — то же и применить: при `error` или конфликтах — `INVALID_INPUT`, иначе сначала копия vault в `.bak`, затем всё одним сохранением |
| `archive_store` | `id, archived` | `()` — каскадно на всё поддерево; восстановление под архивным родителем — ошибка |
| `delete_store` | `id` | `()` — только без операций и без субсчетов |
| `get_store_overview` | `store_id, include_children?` (по умолчанию `true`) | `StoreOverview` — см. `docs/data-model.md`; `unrealized_base`, `subtree_unrealized_base`, `prices_as_of` (самая старая рыночная цена позиций); у `StorePosition` — `change_24h_pct` и `price_info { origin, source, at }` (откуда цена) |
| `get_asset_holdings` | `asset: AssetRef` | `AssetHoldings { asset, base_currency, total_quantity, total_value_base, rows: AssetHolding[] }` — тот же актив (`AssetRef::same_as`: один ключ или один тикер на той же сети) во всех кошельках; `peg_valued` — оценён по привязке $1, без цены |
| `get_store_months` | `store_id, include_children?` (`true`), `months?` (6, до 36) | `Vec<StoreMonthRow { month, inflow, outflow, tx_count }>` — приход и расход кошелька по местным месяцам в базовой валюте, старые первыми; переводы внутри кошелька и его субсчетов не считаются |

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
| `list_transactions_page` | `filter?: TxFilter`, `offset: u32`, `limit: u32` | `TxPage { items: Vec<Transaction>, total }` — страница списка операций (новые сверху) и сколько всего подходит под фильтр; экран «Операции» грузит по 100 |
| `get_tx_graph` | `filter?: TxFilter, collapsed?: string[]` | `TxGraph { currency, lanes[], nodes[], edges[], columns }`. Дорожка — кошелёк в порядке дерева; узел — операция на дорожке (`column` — порядковый номер по времени, `lane` — индекс дорожки, `value` — сумма в базовой валюте на момент операции); рёбра `seq` / `transfer` / `convert` / `flow` всегда идут вперёд по времени. `collapsed` — кошельки, чьи субсчета сворачиваются в их дорожку. Больше 2000 узлов → `TOO_LARGE` |
| `add_transaction` | `TransactionInput` | `Transaction` — после автокатегоризации и проверок V1–V14 |
| `add_linked_transactions` | `AddLinkedTransactionsInput { transactions: Vec<TransactionInput>, flow_id? }` | `Vec<Transaction>` — несколько операций одной цепочкой (`flow_id` общий, новый если не задан): обмен на нескольких получателей, P2P-сделка, пополнение нескольких счетов. Каждая видит балансы предыдущих; всё или ничего |
| `preview_transaction` | `PreviewTransactionInput { transaction, replaces? }` | `TxPreview { changes[{store_id, store_name, asset, before, after, delta_base}], base_currency, flow, warnings[], error? }` — ничего не пишет; `replaces` — id редактируемой операции, её эффект вычитается; отказ валидации приходит в `error` строкой `CODE: message` |
| `update_transaction` | `UpdateTransactionInput { id, transaction: TransactionInput }` | `Transaction` — `id` и `origin` сохраняются, холдинги пересчитываются |
| `delete_transaction` | `id` | `()` — снимает и связь с объектом |
| `tx_property_links` | — | `TxProperty[] { tx_id, property_id }` — какие операции относятся к объекту имущества (поле «Объект», `TxCommonInput.property_id` у `add_transaction`/`update_transaction`; пусто — связь снимается) |
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
| `update_category` | `UpdateCategoryInput { key, name?, group, essential?, passive? }` | `Category` (пустое имя — встроенная подпись; `essential: null` / `passive: null` оставляют флаг обязательной траты / пассивного дохода как есть) |
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
| `create_contact` | `ContactDraft { name, kind, note, accounts, group }` (`group` — группа в справочнике, пусто — по виду) | `Contact` (реквизиты без `id` — новые, активные) |
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
| `get_net_worth` | — | `NetWorth { base_currency, financial, non_financial, receivables, liabilities, liabilities_by_kind: [{ kind, amount }], payables, net, financial_by_class: [{ class_id?, amount }], financial_fund }` — `liabilities_by_kind` раскладывает `liabilities` по видам кредитов, по убыванию; `financial_by_class` раскладывает финансовые активы по корзинам портфеля (`class_id: null` — без корзины), подушка отдельно в `financial_fund` |
| `asset_classes_get` | — | `AssetClassesView { classes, preset, links }` — корзины портфеля с целевыми долями; пока пользователь ничего не сохранил, `preset: true` и отдаётся 40/40/20 (имя пустое — UI показывает своё) |
| `asset_classes_set` | `classes: AssetClass[]` | `AssetClassesView` — заменить корзины; доли 0..100, сумма = 100 (иначе `INVALID_INPUT`), пустой список возвращает пресет; привязки к удалённым корзинам снимаются |
| `asset_class_assign` | `target: { kind: 'asset', asset_key } \| { kind: 'store', store_id }, class_id?` | `AssetClassesView` — корзина актива (важнее корзины счёта) или счёта; `null` снимает; счёт подушки в корзину не ставится; на пресете первое назначение сохраняет пресет |
| `topup_advice` | `amount?` | `TopupAdvice { currency, amount, from_savings, savings_month?, rows: [{ class, value, pct_before?, add, pct_after?, capped }], unclassified, emergency, preset }` — как разделить сумму (по умолчанию накопления прошлого месяца) по корзинам, чтобы доли шли к целевым; корзина с `cap` выше цели не получает ничего. Операций не создаёт |
| `set_manual_price` | `SetManualPriceInput { asset_key, unit_price, currency }` | `ManualPrice` — оценка имущества в узле `asset`, на всю историю |
| `clear_manual_price` | `asset_key` | `()` — назад к цене покупки |
| `property_valuations` | `store_id` | `PropertyValue[]` — по активу объекта: покупка (первая «Приобрести», иначе старые поля формы), оценки по датам, правило, цена сегодня, изменение к покупке (номинал и реально — по инфляции валюты) |
| `property_valuation_add` | `ValuationInput { store_id, asset_key, date, unit_price, currency }` | `Valuation` — оценка на дату; та же дата заменяет прежнюю. Между оценками цена линейна, после последней — правило |
| `property_valuation_delete` | `id` | `Valuation` — удалённая оценка (для «Отменить») |
| `get_history` | `base_currency?` | `Vec<HistoryPoint { ts, ts_ms, value_usd, value_base }>` |
| `get_realized_pnl` | — | `RealizedPnl` |
| `get_store_history` | `store_id, include_children?, base_currency?` | `Vec<HistoryPoint>` — по узлу и (по умолчанию) его субсчетам |
| `get_store_pnl` | `store_id, include_children?` | `RealizedPnl { total_base, fees_base, legs[] }` — по той же области; комиссии отдельно |

### calc
| `calc_preview` | `CalcInput { mode, amount, price_in, price_out, target_price?, stop_price?, fee_pct? }` | `CalcResult { gross_in, fee, spent, gross_out, net_out, exit_value, pnl, roi_pct, risk, risk_pct }` |
| `calc_invest_simulate` | `input: InvestInput { currency, start, initial, rate_pct, term_months, capitalization, stages[], indexation_pct, inflation_pct, tax, withdrawal? }` | `InvestResult { final_nominal, final_real, contributed, invested, interest, tax, withdrawn, effective_rate_pct, crossover_year?, depleted_at?, years[{ …, months[] }] }` |
| `calc_invest_solve` | `input, target, unknown: contribution\|term\|rate` | `SolveResult { reachable, value, input, result }` — при `reachable = false` лучший достижимый вариант |
| `calc_invest_compare` | `input` | `InvestCompare { rows[{ kind: base\|more_rate\|more_contribution, final_nominal, gain }], contribution_wins }` |
| `calc_scenarios_list` | — | `CalcScenario[]`, последний изменённый первым |
| `calc_scenario_save` | `draft: CalcScenarioDraft { id?, name, input, source?, prefilled_at? }` | `CalcScenario`; `id = null` создаёт новый |
| `calc_scenario_delete` | `id` | `CalcScenarioDeleted { scenario, goal_ids }` — цели остаются, связь снимается |
| `calc_scenario_restore` | `deleted: CalcScenarioDeleted` | `CalcScenario` — отмена удаления, связи с целями возвращаются |
| `calc_scenario_prefill` | `source: ScenarioSource { scope, income_months: 3\|6\|12, income_pct }, currency` | `ScenarioPrefill { initial, rate_pct?, capitalization?, pays_out, rate_store_id?, monthly_contribution?, income_avg?, income_months_used, prefilled_at }` — ничего не сохраняет |

### app settings (вне vault, в конфигах приложения)
| `app_platform` | — | `string` — ОС приложения (`linux`, `windows`, `macos`, `android`, `ios`): трей и автозапуск показываются только на ПК; мок читает `?platform=` из адреса |
| `set_system_bars` | `dark: bool` | `()` — цвет значков строки состояния и навигации под тему (Android, плагин `insets`); на ПК ничего не делает |
| `move_to_back` | — | `()` — свернуть приложение (системный «Назад» на Главной, Android); на ПК ничего не делает |
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
| `list_flows` | `from?`, `to?` (RFC3339), `storeIds?` (цепочки, задевшие эти кошельки с подкошельками) | `Vec<FlowSummary { flow_id, currency, links[{ tx, loss }], started_at, ended_at, gave[], got[], gave_value, got_value, loss, difference }>` — новые первыми |
| `link_transactions` | `LinkTransactionsInput { ids, flow_id? }` | `String` — id цепочки: заданный, иначе цепочка одной из операций, иначе новый |
| `unlink_transaction` | `id` | `()` |
| `get_fx_table` | — | `FxTable { base, rows[{ currency, per_base, in_base, updated_at, tracked, crypto, favorite }] }` — последние курсы к базовой; избранные монеты — строкой из последней цены (`crypto`) |

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
| `get_report` | `kind: ReportKind` (`cashflow` \| `year` \| `categories` \| `networth` \| `stores` \| `losses` \| `pnl` \| `portfolio` \| `inflation`), `filter: ReportFilter { scope, from?, to? (YYYY-MM, по умолчанию последние 12 месяцев), year?, currency?, real_terms }` | `Report` (в TS — `AnalyticsReport`, чтобы не путать с DOM-типом `Report`): внешне помеченный вариант с отчётом своего вида и `meta { currency, months, real_terms, inflation_rate?, insights }`; `insights` — факты для вывода словами (`Insight { code, amount?, pct?, subject? }`, текст собирает фронт по `insight.<code>`); у `networth`, `pnl`, `portfolio` — `has_data` (строка или точка есть на каждый месяц, пустоту видно только по флагу) |
| `get_report_overview` | `filter: ReportFilter` (берутся `scope`, `currency`, `real_terms` и месяц `to`, по умолчанию текущий) | `ReportOverview { currency, month, previous_month, year, flow, wealth, investments, savings }`: карточки стартового экрана аналитики — месяц против прошлого, вложения с начала года; у каждой `has_data` и `insights` |
| `capital_health` | — | `HealthReport { color, reasons, currency, emergency, debts, passive, savings, savings_target_pct, savings_avg_pct, creep, freedom_rate }`: чекап капитала — светофор с причинами, подушка, кредиты и DTI, пассивный доход и индекс свободы, норма сбережений за 12 завершённых мес. (валюта бюджета) |
| `health_settings_set` | `expectedReturn: Amount \| null` | `null`; годовая доходность, которую предполагает «потенциал» индекса свободы; `null` — ключевая ставка; отрицательная — `INVALID` |
| `export_report_csv` | те же `kind`, `filter` | `Option<String>` — путь файла; `None`, если диалог закрыли. CSV: UTF-8 с BOM, `;`, CRLF, колонка валюты в каждой строке |
| `get_calendar` | `month?` (`YYYY-MM`, по умолчанию текущий) | `CalendarMonth { month, currency, allowance?, budget?, days[{ date, income, spent, daily, tx_count, status, left?, per_day_left?, notices[] }], upcoming[] }`; `status`: `none` \| `under` \| `near` (> 80 % дневного бюджета) \| `over` \| `future`; `budget` — пул сальдо: доход − **плановые** обязательные − план накоплений (фактические обязательные платежи сальдо не двигают; при ручном дневном лимите меньше плана — лимит × дни); `left` — сальдо: `budget` минус ежедневные траты по этот день включительно, у дней впереди — прогноз (`forecast`): остаток сегодня минус доля дня за каждый день до этого; `per_day_left` — `left` на оставшиеся дни (впереди — сегодняшняя доля); `notices` — записи журнала уведомлений этого дня (`CalendarNotice { key, category, title, at, link? }`, ценовые алерты — `price_alert`) |
| `get_entity_card` | `target: EntityRef { kind, id }` (`kind`: `store` \| `tx` \| `category` \| `contact` \| `goal` \| `budget` \| `automation` \| `workflow` \| `reminder` \| `alert` \| `asset`, у актива `id` = `AssetRef::key()`) | `EntityCard { target, title, value?, currency?, archived, missing, groups[{ kind, items[{ target, label, when?, value?, currency?, archived }], total }], transaction? }`: карточка любой сущности (план J1) — шапка и связи; пустые группы не приходят, в группе не больше 20 записей (`total` — сколько всего); `missing` — сущности нет (удалена или ссылка из другого vault); пустой `title`/`label` — имя даёт фронт (встроенная категория, лимит) |
| `get_search_index` | — | `SearchEntry[] { target: EntityRef, label, subject?: EntityRef, archived }`: всё, что палитра Ctrl+K ищет по имени (план J2) — цели, лимиты, категории, контакты, автоматизации и сценарии, напоминания (кроме выключенных), оповещения, активы с ненулевым остатком, сценарии калькулятора; кошельки фронт берёт из дерева. Пустой `label` — имя даёт фронт (встроенная категория, лимит по `subject`) |
| `search_transactions` | `query`, `categories: string[]` (ключи встроенных категорий, чьё переведённое имя совпало на фронте), `limit` (1..50) | `Transaction[]`, новые первыми: совпадение по комментарию, имени кошелька, контакта, своей категории или по началу суммы (`1 234,5` = `1234.5`) |
| `get_pinned` | — | `PinnedView[] { pin: Pinned, title, value?, currency?, pct?, missing }`: блок «Закреплённое» в сайдбаре; кошелёк — стоимость, цель — накоплено и `pct`, лимит — потрачено и `pct` использования, категория — траты за месяц; у фильтра `title` = его имя |
| `set_pinned` | `pins: Pinned[]` (`{ type: 'entity', target }` \| `{ type: 'filter', id, name, query }`) | `PinnedView[]`: заменяет список целиком, порядок сохраняется; повторы и пустые ссылки отбрасываются, фильтру без `id` даётся uuid, `?` в начале `query` снимается, не больше 30; фильтр без имени — `INVALID_INPUT` |

### planning (`crates/app/src/commands/planning.rs`)

Дни, недели и месяцы режутся в поясе `AppSettings.timezone` (иначе — пояс ОС,
иначе UTC); «сейчас» берёт команда. Суммы бюджета — в валюте бюджета
(`Settings.budget_currency`, пусто — базовая) по курсу на дату операции.

| Команда | Вход | Выход |
|---|---|---|
| `list_automations` | — | `Vec<Automation>` по имени |
| `create_automation` | `AutomationInput { name, active, preset?, schedule, starts_at, template: TransactionInput, amount_mode?, workday_shift? }` (пресеты по типу кошелька — в `get_wallet_catalog`) | `Automation { …, amount_mode, workday_shift, estimate? }`. Автоматизация — схема, построенная из формы (`Workflow.automation`). Шаблон — один из четырёх глаголов и проходит те же проверки, что операция. `amount_mode: varies` (не для обмена) — сумма каждый раз разная: запуск записывает среднее за 3 последние уточнённые суммы (`estimate`, пока их нет — сумма шаблона) с отметкой «примерно». `workday_shift: back/forward` — дата на выходном или празднике РФ переносится на рабочий день раньше/позже |
| `update_automation` | `UpdateAutomationInput { id, automation }` | `Automation`; граф строится заново, прежний — в версиях схемы; обработанные даты остаются обработанными |
| `delete_automation` | `id` | `()` — вместе с историей запусков; созданные операции остаются |
| `run_automation` | `id` | `Transaction` «сейчас»; не хватает средств — `INSUFFICIENT_FUNDS` с суммами |
| `simulate_automations` | `SimulateAutomationsInput { automation_id?, days (1–366) }` | `AutomationSimulation { from, until, runs[{ automation_id, automation_name, planned_at, outcome, transaction? }], balances[{ store_id, asset, before, after }], created, rejected }` — эмуляция: автоматизации прогоняются по дням на копии vault (`UnlockedVault::sandbox`, ничего не пишет) с теми же проверками денег и правил, что у планировщика; с `automation_id` в ответе только её запуски |
| `list_automation_runs` | `automation_id?` | `Vec<AutomationRun>`, новые первыми |
| `retry_automation_run` | `run_id` | `Transaction`: баланс проверяется на текущий момент, дата операции — сейчас |
| `confirm_estimated_amount` | `ConfirmEstimatedInput { tx_id, amount }` | `Transaction` с фактической суммой и снятой отметкой «примерно»; она идёт в следующую оценку. Обычная правка суммы такой операции (`update_transaction`) тоже снимает отметку. Операция без отметки — `INVALID_INPUT` |
| `detach_automation` | `id` | `Workflow` без `automation`: дальше схема правится в редакторе, форма её не показывает. Пока схема — автоматизация, `save_workflow`/`save_workflow_draft` для неё отвечают `INVALID_INPUT` |
| `skip_automation_run` | `run_id` | `AutomationRun` с `Skipped` |
| `list_budgets` / `create_budget` / `update_budget` / `delete_budget` | `BudgetInput { active, target, period, limit, currency }` / `UpdateBudgetInput { id, budget }` / `id` | `Budget` |
| `get_budget_status` | — | `Vec<BudgetStatus { budget, spent, remaining, used, period_start, period_end }>` — активные лимиты, траты групп «обязательные» и «ежедневные» в валюте лимита |
| `get_daily_budget` | — | `DailyBudget { mode, currency, date, per_day, carry, available, spent_today, left_today, income, fixed, savings_plan, free, days, next_income?, manual_limit? }` |
| `get_month_summary` | `month?` (`YYYY-MM`, по умолчанию текущий) | `MonthSummary { income, fixed, daily, savings_plan, savings_fact, deviation, saved_pct, savings_moved, budget_in, budget_out, not_executed, complete }` |
| `list_goals` / `create_goal` / `update_goal` / `delete_goal` | `GoalInput { name, active, target_amount, currency, starts_at, deadline?, priority, monthly_commitment?, scope, scenario_id? }` / `UpdateGoalInput { id, goal }` / `id` | `Goal`; список — по важности, затем по дедлайну |
| `get_goal_progress` | — | `Vec<GoalProgress { goal, saved, contributed, fx_gain, pct, months_total?, months_left?, need_per_month?, realistic, issues, forecast?, accumulating, planned_now? }>` |
| `get_goal_calendar` | `year` | `GoalCalendar { year, currency, months[{ month, need, saved, shortfall, done }] }` |
| `goal_from_scenario` | `id` | `GoalInput` — черновик цели на итог сценария (срок, взнос в месяц, кошельки, `scenario_id`); сохраняет форма цели |
| `emergency_fund_get` | — | `EmergencyStatus` — подушка: счета, сумма, обязательные траты в месяц (`e_mand`), покрытие в месяцах, цель, излишек, проценты за 12 мес., счета-кандидаты (валюта бюджета) |
| `emergency_fund_set` | `fund: EmergencyFund { scope, target_months, avg_months }` | `EmergencyFund`; только живые счета «Сбережения», месяцы 1–24, пустой список = подушки нет |
| `emergency_fund_toggle` | `storeId`, `on` | `EmergencyFund` — добавить счёт в подушку или убрать |
| `list_reminders` / `create_reminder` / `update_reminder` / `delete_reminder` (`list_reminders` сначала синхронизирует авто-напоминания) | `ReminderInput { text, due_at (RFC3339: дата и время), repeat?: { every, unit: day\|week\|month\|year } }` / `UpdateReminderInput { id, reminder }` / `id` | `Reminder { …, repeat?, source?, muted, user_edited, snoozed_until? }`; правка снимает откладывание, у авто-напоминания ставит `user_edited` |
| `set_reminder_done` | `id, done` | `Reminder` — у повторяющегося «выполнено» не закрывает, а переносит `due_at` на первую дату серии после «сейчас» (отсчёт от исходной даты: 31-е остаётся 31-м, в коротком месяце — последний день) |
| `finish_reminder_series` | `id` | `Reminder` — завершить серию: `repeat = None`, `done`; авто-напоминание ещё и `muted`, чтобы синхронизация его не вернула |
| `snooze_reminder` | `SnoozeReminderInput { id, until }` | `Reminder` — не наступает раньше `until` (`snoozed_until`) |
| `list_price_alerts` / `create_price_alert` / `update_price_alert` / `delete_price_alert` | `PriceAlertInput { asset: AssetIdent, condition, price, currency, active, store_id? }` / `UpdatePriceAlertInput { id, alert }` / `id` | `PriceAlert`; правка взводит алерт заново. `store_id` — биржа, из терминала которой заведён алерт (только корневой кошелёк `exchange`, иначе `INVALID_INPUT`) |
| `list_utility_items` / `create_utility_item` / `update_utility_item` / `delete_utility_item` | `UtilityInput { name, active, kind: metered{ unit, tariff, formula?, zones[] } \| static{ amount } \| housing{ amount, deposit?, due_day, automation_id? }, currency, property_id, store_id?, category }` / `UpdateUtilityInput { id, item }` / `id` | `UtilityItem` — ЖКХ всегда принадлежит имуществу (`property_id` — корневой кошелёк `property`, иначе `MISSING`); удаление уносит показания, платежи остаются. `formula` — своя формула счёта (`utilities_formula`: `+ - * / ()`, сравнения, `мин/макс/округл/если` через `;`, имена `показание/предыдущее/расход/тариф/дней` и латинские синонимы, у зон — `расход_<зона>`), с формулой тариф может быть 0; ошибка в формуле — `INVALID_INPUT` с номером символа. `zones` — до 4 зон многотарифного счётчика |
| `add_meter_reading` | `MeterReadingInput { item_id, value, period (YYYY-MM), day?, zone? }` | `MeterReading`; `day` — день месяца (1–31, `None` = конец месяца), в месяце может быть несколько показаний (переезд); показание за тот же месяц и день заменяется; меньше более раннего или больше более позднего — `INVALID_INPUT`; у счётчика с зонами `zone` обязательна, порядок проверяется внутри зоны, месяц считается, когда прочитаны все зоны |
| `delete_meter_reading` | `id` | `()` |
| `get_property_summary` | `storeId` | `PropertySummary { value_base, debt_base, equity_base, utilities_month_base, loan_month_base, monthly_cost_base, utility_count, rent_12m_base, expenses_12m_base, rent_yield_pct?, ownership_month_base?, wear_12m_base }` — долг по кредитам с `secured_by` на объект; за 12 мес.: аренда (связанные пополнения), расходы (связанные операции, ЖКХ объекта, платежи по кредиту под залог), доходность аренды к стоимости, у машины — износ и владение в месяц |
| `get_meter_trend` | `item_id` | `UtilityTrend { months[{ period, reading?, consumption?, amount, readings[] }], average?, delta_pct?, forecast?, anomaly, monthly }` — `reading` — показание на конец месяца, `readings` — все показания месяца по дням; месяц считается от последнего показания до него, первый месяц счётчика — от его первого показания; среднее за 6 месяцев со счётом, прогноз — линейная регрессия, аномалия — > 1.5× среднего прежних |
| `get_utility_payment_draft` | `item_id`, `period?` (по умолчанию текущий месяц) | `UtilityPaymentDraft { transaction (payment), consumption?, includes_deposit }` — форма «Оплатить» открывается с ним; комментарий = название счёта, категория счёта |
| `update_meter_reading` | `UpdateMeterReadingInput { id, value, period, day? }` | `MeterReading` — исправить показание; те же правила монотонности, id сохраняется, при отказе старое значение остаётся |
| `get_utility_bill` | `property_id`, `period?` | `UtilityBill { pay_day?, due_at, lines[{ item, previous_reading?, reading?, consumption?, amount?, paid, includes_deposit }] }` — счёт ЖКХ объекта за месяц; `amount = null` у счётчика без показания |
| `preview_utility_formula` | `UtilityFormulaInput { tariff, formula?, zones[], readings[{ zone?, previous, current }], days? }` | `UtilityFormulaPreview { amount?, consumption?, issue?: FormulaIssue { position, text_ru, text_en } }` — счёт по формуле (или расход × тариф) для введённых показаний; vault не нужен. Строка счёта (`UtilityBillLine.zones`) и месяц тренда (`UtilityMonth.zones`) несут показания по зонам |
| `pay_utility_bill` | `PayUtilityBillInput { property_id, period, store_id, timestamp?, lines[{ item_id, amount, reading?, reading_day?, zone_readings[{ zone, reading }] }] }` | `Vec<Transaction>` — показания (на день `reading_day`, пусто — конец месяца) + по платежу на строку с `meta.utility_item_id`; всё или ничего |
| `set_deposit_terms` | `SetDepositTermsInput { store_id, terms? }` | `Store` — ставки (история), капитализация, срок, автоначисление, ПДС; только сабсчета `savings`/`deposit`/`pds` банка |
| `get_deposit_report` | `store_id` | `DepositReport { balance, rate?, accrued_month, accrued_year, posted_year, projected_year, matures_at?, at_maturity?, pds? }` — проценты по дням по фактическому балансу |
| `get_interest_tax` | `year?` | `InterestTaxReport { max_key_rate?, limit?, income, taxable, tax, remaining_free? }` — НДФЛ 13% сверх 1 000 000 ₽ × макс. ключевая ставка на 1-е числа года |
| `refresh_rates` | — | `Vec<InflationCurrency>` — сейчас забрать ключевую ставку ЦБ РФ (SOAP `DailyInfo.KeyRate`, с 1 января прошлого года, в `VaultDoc.key_rates`) и инфляцию г/г за 25 мес. (RUB — таблица Банка России `hd_base/infl`, USD — BLS CPI-U, EUR — Eurostat HICP `prc_hicp_minr`) в `VaultDoc.inflation_points`; недоступный источник пропускается, `NETWORK` — если не ответил ни один. Новые цифры — уведомления категории `rates`, затем `rates-updated`. То же делает планировщик раз в сутки (при ошибке — через час) |
| `get_inflation_overview` | — | `Vec<InflationCurrency { currency, mode, chosen, source?, latest?, previous?, manual?, effective?, series[] }>` — по каждой валюте со встроенным источником или ручным числом: режим (`chosen` — выбран ли он явно), последняя и предыдущая цифра, ручное значение из `AppSettings.inflation`, `effective` — что используют отчёты и калькулятор (в `auto` до первой цифры — ручное) |
| `set_inflation_mode` | `currency`, `mode?` (`auto` \| `manual` \| `source`, `null` — по умолчанию) | `Vec<InflationCurrency>` — режим в `Settings.inflation_mode`; затем `rates-updated` |
| `set_inflation_source` | `currency`, `sourceId` | `()` — инфляция валюты читается из своего источника: `Settings.inflation_source[currency]`, режим `source`; затем `rates-updated` |
| `datasource_preview` | `url` | `SourcePreview` — страница для окна выбора: HTML без скриптов, обработчиков, форм и фреймов, с CSP и `<base>` (`kind = html`), JSON-текст (`json`) или первые 200 строк CSV/XLSX (`rows`). Размер страницы — до 8 МБ |
| `datasource_test` | `draft: DataSourceDraft` | `Extracted` — значение, которое метод находит на странице сейчас; ничего не сохраняет. Ошибка разбора — `INVALID_INPUT` |
| `datasources_list` | — | `Vec<DataSourceView>` — свои источники с местами использования (`used_by`: инфляция, цена актива, ставка вклада, сценарии) |
| `datasource_save` | `draft: DataSourceDraft` | `DataSourceView` — создать/изменить; `uses` заменяет инфляцию, цены и вклады источника. Новый источник сразу забирается. Затем `sources-updated` |
| `datasource_delete` | `id` | `()` — удалить; инфляция, читавшая его, возвращается к режиму по умолчанию |
| `datasource_refresh` | `id` | `DataSourceView` — забрать сейчас. Ошибка не меняет значение, 3 подряд — `broken` и уведомление; скачок больше 10 раз — `pending` до `datasource_confirm`. То же делает планировщик по интервалу источника |
| `datasource_confirm` | `id`, `accept` | `()` — принять или отбросить значение, ждущее подтверждения после скачка |
| `set_loyalty_info` | `SetLoyaltyInfoInput { store_id, info: LoyaltyInfo }` | `Store` — программа бонусов; цена балла становится ручной ценой актива баллов |
| `create_loyalty_program` | `CreateLoyaltyProgramInput { name, color, contact_id?, info: LoyaltyInfo }` | `Store` — программа бонусов создаётся сразу с баллами (`login`, `source_store_ids`); отказ проверки не оставляет кошелька |
| `adjust_points` | `AdjustPointsInput { store_id, delta, comment, source_store_id?, timestamp? }` | `Transaction` — начисление (topup, категория `cashback`) или списание баллов |
| `get_cashback_report` | `period?` (`month` \| `year`) | `CashbackReport { cashback_base, points_base, by_source[], programmes[] }` |
| `get_upcoming` | `days?` (30, не больше 366) | `Vec<UpcomingItem { kind, source_id, at, date, title, store_id?, amount?, asset?, shortfall?, estimated }>` — автоматизации с прогнозом «не хватит N» (даты уже перенесены с выходных, у разной суммы — оценка и `estimated`), аренда, платежи по кредитам, напоминания, отклонённые запуски (`rejected_run`) и операции, записанные примерно (`unconfirmed_run`, `source_id` — id операции для `confirm_estimated_amount`) |

### workflows — схемы из узлов (`crates/app/src/commands/workflows.rs`)

Граф хранится в vault как JSON (`WorkflowGraph { version, nodes, edges, comments }`), каталог узлов — в Rust (`trade_core::workflow::catalog`). Схема с ошибками сохраняется, но не включается. Планировщик раз в тик вызывает `run_due_workflows`: дождавшиеся «Подождать», расписания (пропущенное — не больше 12), новые операции (свои операции схемы её не запускают; операции других схем — только с `include_workflows` / `react_to_workflows`), пересечения порогов баланса и цены, затем события (глубина ≤ 5). Узел «Вызвать схему» выполняет схему с триггером «Вызов из другой схемы» внутри того же запуска (общие операции, лимиты и откат, вложенность ≤ 5), ответ отдаёт «Вернуть значение». Лимиты: 500 шагов и 20 действий за запуск, 60 запусков схемы в час, `max_amount` в базовой валюте. Неудачный запуск откатывает всё, что сделал.

#### Как выполняется автоматизация (граф v3)

- **Поток и данные.** Провод потока задаёт, что выполняется дальше; провод данных несёт значение. Данные считаются лениво, когда узел их просит, и запоминаются на запуск (после узлов записи и на каждом проходе цикла — заново).
- **Порядок по данным.** Действие без входящего провода потока, которое берёт данные у других действий (напрямую или через чистые узлы), выполняется сразу после того, как в этом проходе выполнились все они (`Graph::data_order`). Если источник не выполнился (другая ветка «если»), не выполняется и оно. Поток нужен только для действий без результата, веток, ожиданий и циклов. `validate` предупреждает: `redundant_flow` — единственный поток `out → in` повторяет связь по данным; `late_data` — узел читает действие, до которого поток доходит позже. Граф v2 при загрузке мигрирует в v3 и выключает узлы, которые по новому правилу «ожили» бы (раньше они не выполнялись).
- **Ветки.** Несколько следующих узлов — это ветки (явные провода и узлы по данным). Перед каждой — точка сохранения; необработанная ошибка в ветке откатывает только её, остальные идут дальше, исход — `partial { failed }`. Ошибка на основной линии откатывает весь запуск (`failed`).
- **Ошибки.** У всех действий и узлов переменных есть выход «Ошибка» и «Причина» (кроме «Стоп» и «Вернуть значение»): подключённый — ловит сбой без отката.
- **Сон.** «Подождать» и «Ждать подтверждения» сохраняют `WorkflowWait` с запомненными значениями и копией графа (`graph`): продолжение идёт по тому графу, на котором запуск уснул; чистые данные пересчитываются, а «свободные» узлы выполняются в конце, после последнего ожидания. Ответ на вопрос выключенной схемы или сверх лимита — `INVALID_INPUT`, вопрос остаётся. Ожидание сверх лимита ждёт следующего тика; отброшенное (схема выключена, узел удалён) и запуск сверх 60 в час попадают в журнал с исходом `dropped { reason: off | node_removed | rate_limit }` и в лимит не считаются.
- **Узел «Очередь»** (`flow.queue`). Если в схеме есть включённая «Очередь», все срабатывания одного триггера за проход планировщика (новые операции, пропущенные моменты расписания, одинаковые события) дают **один** запуск с `RunContext.batch` (метка `… ×N`, в лимит — как один). Узлы до «Очереди» видят первое срабатывание; «Очередь» по каждому выставляет значения триггера, `item`/`index`/`count` и проходит `each`, затем `done`. `mode: sequence` — сбой откатывает весь запуск; `independent` — каждое срабатывание своя ветка (`partial`). «Запустить»/эмуляция дают пакет из последних до 3 подходящих операций. `validate`: `queue_without_trigger` (ошибка) и `queue_sleeps` (ожидание внутри `each`).
- **Очередь подтверждения** (`queue_mode`) больше не заполняется: её роль — у узла «Ждать подтверждения». Поле и команды `*_workflow_queued` остаются для записей старых версий.

| Команда | Вход | Выход |
|---|---|---|
| `workflow_node_catalog` | — | `Vec<NodeDef { kind, group, title, description, help, inputs, outputs, params, advanced, keywords }>`; тексты `{ ru, en }`; работает и без открытого vault |
| `validate_workflow` | `ValidateWorkflowInput { graph }` | `Vec<GraphIssue { level, code, node_id?, edge_id?, param?, text_ru, text_en }>`, ошибки первыми; `param_missing` — пустое обязательное поле (`ParamDef.required`, `REQUIRED_PARAMS`), `param` — его имя. Включённая схема с таким полем выключается при открытии хранилища (журнал + уведомление) |
| `list_workflows` | — | `Vec<Workflow>` |
| `get_workflow` | `id` | `Workflow` |
| `save_workflow` | `SaveWorkflowInput { id?, workflow: WorkflowDraft { name, description, active, graph, react_to_workflows, max_amount? } }` | `SavedWorkflow { workflow, issues }`. Изменённый граф кладёт прежний в `versions` (до 20); включение начинает следить «с этого момента» |
| `set_workflow_active` | `SetWorkflowActiveInput { id, active }` | `Workflow`; с ошибками в графе — `INVALID_INPUT` |
| `delete_workflow` | `id` | `()` — вместе с журналом, своими переменными, очередью, состоянием и ожиданиями; созданные операции остаются |
| `restore_workflow_version` | `RestoreWorkflowVersionInput { id, saved_at }` | `SavedWorkflow`; текущий граф становится версией, сломанная версия возвращается выключенной |
| `dry_run_workflow` | `DryRunWorkflowInput { graph, workflow_id?, trigger_node?, answers? }` | `RunReport { run, transactions, queued, notices, events, reminders, event_runs, edge_values }` — эмуляция на копии vault, ничего не пишется; «Подождать» в ней сразу идёт дальше, но сдвигает часы запуска (даты и напоминания после него — как в реальном запуске); `event_runs` — запуски схем, которые стартовали бы от её событий, на той же копии; `edge_values` — значение на каждой связи данных для редактора; узел «Ждать подтверждения» без ответа в `answers` (node id → да/нет) останавливает эмуляцию с исходом `asking` |
| `run_workflow_now` | `RunWorkflowInput { id, trigger_node? }` | `RunReport`; уведомления показываются, запуск попадает в журнал |
| `list_workflow_runs` | `ListWorkflowRunsInput { workflow_id?, limit? (100) }` | `Vec<WorkflowRun>` новыми первыми, с `trace` по шагам |
| `list_workflow_queue` | — | `Vec<QueuedActions>` — операции, поставленные в очередь старым узлом «Ждать подтверждения» (до графа v2) |
| `save_workflow_draft` | `SaveWorkflowDraftInput { id, graph }` | `Workflow` — откладывает несохранённые правки редактора в `draft { saved_at, graph }`; сохранённый граф и запуски не трогает |
| `discard_workflow_draft` | `id` | `Workflow` — удаляет черновик; `save_workflow` удаляет его сам |
| `list_workflow_waits` | — | `Vec<WorkflowWait>` — уснувшие запуски: `kind: timer` («Подождать») и `kind: confirm` (вопрос «Ждать подтверждения», `question`, `resume_at` — срок или пусто) |
| `answer_workflow_wait` | `AnswerWorkflowWaitInput { id, yes }` | `RunReport` — ответ на вопрос: запуск сразу продолжается по ветке «Да»/«Нет»; шлёт `workflows-ran` |
| `confirm_workflow_queued` | `id` | `Vec<Transaction>` — все операции пачки вместе или ни одной |
| `discard_workflow_queued` | `id` | `()` |
| `list_workflow_vars` | — | `Vec<WorkflowVar { scope, name, value (WfValue JSON), updated_at }>` |
| `set_workflow_var` | `SetWorkflowVarInput { scope, name, value }` | `()` |
| `delete_workflow_var` | `DeleteWorkflowVarInput { scope, name }` | `()` |
| `export_workflow` | `id` | `string` — файл `{ format: "trade-app-workflow", version, name, description, graph }` |
| `import_workflow` | `json` | `SavedWorkflow` — новая схема, выключена |
| `workflow_templates` | — | `Vec<WorkflowTemplateInfo { id, fields: Vec<TemplateField { key, kind (store/category/number/currency/budget/weekday/day), required, default }>, moves_money }>` — галерея шаблонов, умолчания из vault (доля накоплений, валюта бюджета) |
| `workflow_from_template` | `WorkflowFromTemplateInput { template_id, params: { ключ поля: значение } }` | `SavedWorkflow` — готовая схема, включена; переводы денег только после «Спросить» |
| `save_workflow_test` | `SaveWorkflowTestInput { workflow_id, test: WorkflowTest { id (пусто — новая), name, trigger_node?, at?, expect: TestExpect { outcome, transactions?, queued?, notices? } } }` | `Workflow` |
| `delete_workflow_test` | `DeleteWorkflowTestInput { workflow_id, test_id }` | `Workflow` |
| `run_workflow_tests` | `id` | `Vec<WorkflowTestResult { test_id, name, passed, problems_ru, problems_en, report }>` — каждая проверка эмуляцией на копии vault в момент `at` (или сейчас); ничего не пишется |

### settings (vault)
| `get_settings` | — | `Settings` — только vault-scoped поля |
| `update_settings` | `UpdateSettingsInput` | `Settings` (`savings_rate` 0..1; `daily_budget_mode`; `budget_currency`, пусто — базовая). Логика — в core (`UnlockedVault::update_settings`); `notifications: NotificationSettings { system, hide_details_when_locked, default_time, categories[{ category, enabled, lead_days (0–90), auto_reminders }] }` |
| `list_notifications` | — | `Vec<NoticeRecord>` — журнал показанных уведомлений, новые сверху (не больше 500) |
| `mark_notifications_read` | `keys?: Vec<String>` | `()` — отметить прочитанными названные или все |
| `send_test_notification` | — | `()` — показать пробное уведомление (системное — даже если они выключены) |
| `notification_permission` | `request: bool` | `"granted" \| "denied" \| "prompt"` — разрешает ли система уведомления; `request` — спросить |
| `notification_storage` | — | `"keyring" \| "memory"` — где живёт расписание уведомлений для заблокированного vault: зашифрованный файл с ключом в системной связке ключей или только память |
| `get_autostart` / `set_autostart` | — / `enabled: bool` | `bool` — запуск при входе в систему (`tauri-plugin-autostart`, аргумент `--hidden`: окно не показывается, приложение в трее) |
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
| `import_bybit_history` | `ImportBybitHistoryInput { store_id, category?, limit? }` | `ImportBybitHistoryOutput { staged, notice?, run? }` — синхронизация «сейчас»: скачивает сделки, переводы и Earn, отбрасывает уже виденные (в staging, проведённые, отброшенные) и не нужные по настройкам импорта, **кладёт новые в staging** (или проводит сразу при `auto_confirm`), затем при расхождении ставит живой баланс корректировкой; `run` — запись в истории синхронизаций |
| `get_import_settings` | `store_id` | `ImportSettings { trades, transfers, earn, auto_confirm, auto_reconcile, coins, since?, categories{trades?,transfers?,earn?}, accounts[{account, store_id?}] }` — что и куда импортирует биржа |
| `preview_bybit_import` | `SetImportSettingsInput { store_id, settings }` | `ImportPreviewOutput { preview?: { fetched, known, filtered, new, items }, notice? }` — что загрузила бы синхронизация с этими (в т. ч. несохранёнными) настройками; ничего не пишет |
| `set_import_settings` | `SetImportSettingsInput { store_id, settings }` | `ImportSettings` — сохраняет; счета маршрутов — только этот кошелёк или его субсчета, `store_id: null` = счёт биржи не импортируется |
| `list_import_runs` | `store_id` | `Vec<ImportRun { id, at, source, fetched, staged, known, filtered, confirmed, reconcile_tx_id?, error? }>` — история синхронизаций, новые сверху |
| `list_imported_transactions` | `ListImportedTransactionsInput { store_id, limit?, offset? }` | `ImportedTransactions { total, items }` — операции с `origin: import` этого кошелька и субсчетов, новые сверху |
| `list_pending_imports` | — | `ListPendingImportsOutput { imports: Vec<PendingImport> }` (новые сверху) |
| `confirm_pending_imports` | `ConfirmPendingImportsInput { exec_ids }` (пусто = все) | `ConfirmPendingImportsOutput { confirmed }` — `kind = fill`: спот → `TradeSpot`, linear/inverse → `TradeFutures` (комиссия в валюте расчёта), на субсчёт из ключа, иначе Unified; `transfer` → `Transfer` между субсчетами Funding/Unified/Earn; `earn_income` → `Topup` (`deposit_interest`) на Earn; не прошедшая проверку остаётся в staging |
| `discard_pending_imports` | `DiscardPendingImportsInput { exec_ids }` (пусто = все) | `DiscardPendingImportsOutput { discarded }` |
| `create_bybit_accounts` | `CreateBybitAccountsInput { store_id, funding_name, unified_name, earn_name, detect }` | `Vec<String>` — субсчета Funding / Unified / Earn (недостающие создаются). `detect`: сперва спросить у Bybit балансы и создать только счета, где есть монеты (Unified — всегда); без сети создаются все три. Фронт вызывает его с `detect` сразу после `set_exchange_key` (настройки биржи и мастер создания биржи) |
| `bybit_balances` | `store_id` | `Vec<ExchangeAccountBalance { account, store_id?, rows[{ coin, exchange, ledger, diff }] }>` — балансы FUND/UNIFIED/EARN по API рядом с учётом субсчёта; если всё проведено и учёт расходится, сначала ставит живой баланс корректировкой (`auto_reconcile`) |

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