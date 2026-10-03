# План: exchanges в vault (сброс v1) + вложенность кошельков + выбор пути/последние vaults + dev-просмотр

Согласовано с пользователем:
1. Ключи — **по узлу-бирже**, внутри `exchanges`: `exchanges.bybit = [{ store_id, api_key, api_secret }]`.
   Вложенность: ключи **только на узле-бирже (родителе)**, дочерние счёты — агрегация без ключей;
   мультиаккаунт одной биржи = несколько узлов-бирж (каждый со своими ключами).
   Валидация: узел существует, `kind == exchange`, без предка с `kind == exchange`
   (подробнее — A3); при поиске ключа для дочернего счёта — подъём по предкам к узлу-бирже.
2. Путь vault — **поле ввода + кнопка «Обзор…»** (tauri-plugin-dialog).
3. **DEV-кнопка «Открыть vault»** в Настройках: расшифрованный JSON, отформатированный, с подсветкой.
4. **Обратная совместимость ломается**: `VAULT_FORMAT_VERSION` сбрасывается в 1, старые файлы → `UNSUPPORTED`.
5. **Вложенность кошельков** (новая фаза N):
   - свободное дерево по `Store.parent_id` (любая глубина, защита от циклов);
   - holdings/операции висят на **любом** узле как сейчас; родитель = сумма поддерева (roll-up в UI);
   - сайдбар — **дерево с раскрытием** (chevron), у родителя сумма поддерева.

Текущее состояние working copy: `vault.rs` частично отредактирован (добавлен `Exchanges`),
код **не компилируется** (commands/exchange.rs и storage тесты ещё на `doc.bybit`).
Случайно добавлен артефакт `libtest_tauri.rlib` — удалить (`jj file untrack`).

---

## Phase A — схема vault v1 + ключи по кошелькам (чинит компиляцию)

### A1. core/model/vault.rs
- `ExchangeKeys` += `store_id: String` (Debug-redact/hint/`Drop` zeroize — без изменений логики).
- `Exchanges { #[serde(default)] bybit: Vec<ExchangeKeys> }` — **без** `ts_rs::TS` derive
  (секреты не должны попадать в ipc.gen.d.ts; поле в VaultDoc и так `#[ts(skip)]`).
- `VAULT_FORMAT_VERSION = 2 → 1` (crates/core/src/lib.rs; container остаётся 1).
- Удалить «Frozen v1 shapes»: `VaultDocV1`, `SettingsV1`, `upgrade()`, комментарии про prefix/leftover.

### A2. core/storage/mod.rs
- `decode`: JSON — прямой `from_slice::<VaultDoc>`; принимать только `doc.version == 1`,
  иначе `Error::Unsupported` (без «проставления текущей версии»).
- `decode_bin`: убрать v1/leftover-пробу → прямой decode `VaultDoc`.
- Обновить doc-комментарии (v1/v2/migration).
- Тесты: удалить модуль `migration_tests`; сохранить/адаптировать redaction и
  `future_payload_version_is_rejected`; добавить roundtrip (JSON+Bin) с несколькими
  bybit-аккаунтами и отказ на version 2.

### A3. crates/app/commands/exchange.rs
- Все обращения → `v.doc.exchanges.bybit` (Vec).
- **Правило узла-биржи** (решение по вложенности): ключи только на узле `kind == exchange`
  без предка с `kind == exchange` (узел-биржа); дочерние счета ключей не имеют.
- `set_exchange_key`: вход `store_id` + `api_key` + `api_secret`; валидация правила выше;
  замена записи store_id (upsert).
- `clear_exchange_key`: вход `store_id` (удаляет только свою запись).
- `exchange_key_status`: вход `store_id` → configured/hint этого узла.
- `bybit_executions`: вход `store_id?`; выбор ключа: по store_id → если узел без ключа,
  **подъём по предкам к узлу-бирже**; иначе если ровно одна запись — она;
  иначе notice `no_key`.
- `exchange_key_status`/`key_for` — та же логика подъёма по предкам (страница дочернего
  счёта видит, что ключ настроен у родителя-биржи).

### A4. ipc_contract.rs + gen-types
- Поля `store_id` в input-типах, новые input-типы (ClearExchangeKeyInput,
  ExchangeKeyStatusInput) со `#[derive(TS)]`; `mise run gen-types`, задiffить ipc.gen.d.ts.

### A5. Фронтенд (ключи)
- `stores/tradeLookup.ts`: `loadKeyStatus(storeId)`, `saveKey(storeId, …)`,
  `clearKey(storeId)`, `loadExecutions(…, storeId?)` — key в контексте кошелька.
- `AddTransactionForm.vue`: передавать `props.storeId` в status/executions.
- `SettingsView.vue`: убрать карточку «Bybit — импорт сделок».
- `WalletView.vue`: секция Bybit API-ключа — только для **узла-биржи** (правило A3),
  иначе скрыта; ввод, сохранить, удалить, hint `abcd…wxyz`.
- `lib/ipc.ts` mock под новые входы; i18n ru/en: `wallet.bybit.*`, почистить
  неиспользуемые `settings.bybit.*` (паритет локалей).

### A6. docs
- data-model.md: раздел «Ключи бирж» → `VaultDoc.exchanges` (ключи на узлах-биржах), версия 1.
- ipc.md: обновить строки команд (store_id).

---

## Phase N — вложенность кошельков (свободное дерево)

Решения: свободное дерево по `parent_id`; данные на любом узле, родитель = сумма
поддерева; сайдбар — дерево с раскрытием; ключи — только на узле-бирже (см. A3).

### N1. core/model/store.rs
- `Store += pub parent_id: Option<String>` (`#[serde(default)]`); `Store::new` → `None`.
- Схема свежая (v1), миграция не нужна.

### N2. core/service.rs
- `create_store(...)`: параметр `parent_id: Option<String>` — родитель существует (иначе
  `Missing`); имя уникально как сейчас (глобально).
- `update_store(...)`: параметр `parent_id: Option<String>` — перенос узла; **защита от
  циклов**: подъём по предкам нового родителя, если встретили сам узел → `InvalidInput`
  (это же покрывает «сделать ребёнком собственного потомка»).
- `delete_store`: + отказ, если есть дети («архивируйте вместо удаления») — рядом с
  существующей проверкой на операции.
- Хелперы: `children_of(id)`, `subtree_ids(id)` (BFS по `parent_id`).
- `store_overview`: DTO += `children: Vec<StoreChildRow { store, value_usd, value_base }>`
  (прямые значения детей) и `subtree_total_base: Amount` (собственный итог + все потомки).
  `positions`/`tx_count`/`realized_pnl` остаются **прямыми** (данные висят на узле).
- `PortfolioSummary.by_store` — остаётся плоским (прямые holdings по узлам);
  roll-up поддерева делает фронт по `parent_id` (достаточно list_stores + by_store).

### N3. Команды/DTO (crates/app + ipc_contract)
- `CreateStoreInput += parent_id: Option<String>`, `UpdateStoreInput += parent_id: …`;
  `StoreOverview`/`StoreChildRow` расширяются → `mise run gen-types`.

### N4. Фронтенд
- `SidebarWallets.vue` → дерево: рекурсивный рендер, chevron-раскрытие (expanded ids в
  localStorage), отступы, значение = roll-up поддерева (summary.by_store + дерево stores),
  архивный узел скрывает своё поддерево.
- `StoresView.vue`: «Родитель» (select, необязательно) в диалоге создания; показ карточек
  деревом (корни, дети с отступом).
- `WalletView.vue`: хлебные крошки (путь к родителю), секция «Дочерние кошельки»
  (карточки → переход), при наличии детей под итогом hint `subtree_total_base`,
  кнопка «+ Дочерний кошелёк» (create с parent_id).
- Ключи (A5): секция видна только для узла-биржи.

### N5. Тесты core
- создание с родителем; цикл при переносе → ошибка; удаление узла с детьми → ошибка;
  `subtree_total`/`children` roll-up; `by_store` плоский не изменился.

### N6. docs
- data-model.md: `Store.parent_id`, семантика дерева (прямые значения vs subtree),
  правило узла-биржи для API-ключей.

---

## Phase B — путь vault + последние открытые

### B1. Rust (crates/app)
- Модуль рекентов: `app_config_dir()/recent_vaults.json` = `[{ path, last_opened }]`,
  cap 10, дедуп, сортировка по дате (только пути — секретов нет).
- Новые команды:
  - `recent_vaults() -> Vec<RecentVault>` (RecentVault: path, last_opened, exists);
  - `select_vault(path) -> VaultStatus` — обновляет `AppState.vault_path`
    (отказ, если vault разлочен; на экране unlock он всегда locked);
  - `forget_recent_vault(path)`.
- `create_vault`/`unlock_vault` при успехе: писать `vault_path = input.path` и добавлять
  в рекенты (чинит рассинхрон: status показывал default-путь).
- `tauri-plugin-dialog = { workspace = true }` в crates/app + `.plugin(tauri_plugin_dialog::init())`
  + capability `dialog:default` (проверить имя по gen/schemas).
- `RecentVault` DTO → gen-types; команды в `generate_handler!` и docs/ipc.md.

### B2. Фронтенд
- `pnpm add @tauri-apps/plugin-dialog`.
- `lib/ipc.ts` mock: `recent_vaults`, `select_vault`, `forget_recent_vault`.
- `stores/vault.ts`: recents + `select(path)` + `forget(path)`.
- `UnlockView.vue`: шаг 1 — список последних (basename + путь + дата) → клик =
  `select_vault` → шаг 2 ввод пароля; кнопка «Другой путь…» → поле пути
  (input + «Обзор…»: `open()` для существующего, `save()` при создании).
  Ошибка `UNSUPPORTED` → дружелюбный текст «файл старого формата».
- i18n ru/en: `auth.recents`, `auth.otherPath`, `auth.browse`, `auth.unsupported`…

---

## Phase C — DEV «Открыть vault» в Настройках
- Команда `dev_dump_vault` → `serde_json::to_string_pretty(&v.doc)`; в release → `FORBIDDEN`.
- SettingsView: кнопка видна только при `import.meta.env.DEV` → панель/диалог с `<pre>`;
  подсветка — маленькая `highlightJson()` (escape + regex: ключи/строки/числа/булевы/null,
  span-классы на tailwind), без новых зависимостей.
- i18n ru/en, docs/ipc.md.

---

## Phase D — ранее открытые задачи (после A → N → B → C зелёных)
1.2 отступы/spacing → 1.3 вложенные модалки → 1.6 дашборд-виджеты →
2.1 lightweight-charts → 2.2 Strategy Lab → 2.3 tx_hash/network → 2.4 авто-экспорт (на базе A).

## Housekeeping / гейт
- Удалить `libtest_tauri.rlib` (`jj file untrack` + delete, дописать .gitignore).
- Порядок проверки: `cargo fmt` → `mise run gen-types` (после DTO) → `mise run check`.
- Коммиты — только по явной просьбе пользователя.

## Риски
- Старые vault-файлы перестают открываться (`UNSUPPORTED`) — осознанный компромисс,
  компенсируется сообщением в UI.
- bincode-файлы старого формата падают с `Deserialize` вместо `Unsupported` — тоже ок.
