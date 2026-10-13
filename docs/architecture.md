# Архитектура

## Обзор

Multi Wallet Tracker (MWT) — это Tauri 2 desktop-приложение с монолитным Rust-ядром и Vue-фронтендом,
связка через IPC (invoke + events). Вся бизнес-логика живёт в `crates/core`
(чистый Rust, ноль зависимостей от Tauri) и покрыта юнит-тестами; `crates/app`
представляет собой тонкую оболочку: `tauri::Builder`, команды и состояние.

```
┌─────────────────────────────────────────────────────┐
│                    Vue 3 frontend                    │
│  Pinia (состояние) · TanStack Query (сеть+кэш)      │
│  shadcn-vue (UI) · vue-echarts (графики)            │
│  vue-i18n (ru/en) · types из src/shared/api/ipc.gen.d.ts        │
└──────────────────────────┬──────────────────────────┘
                           │ invoke / events
┌──────────────────────────┴──────────────────────────┐
│                crates/app (Tauri shell)             │
│  AppState { vault: Mutex<Option<UnlockedVault>>,    │
│             vault_path: Mutex<PathBuf>, pricing }   │
│  commands/{auth,stores,transactions,categories,     │
│             pricing,portfolio,settings,exchange,     │
│             app_settings,vault_recents}             │
└──────────────────────────┬──────────────────────────┘
                           │ (общая зависимость через workspace)
┌──────────────────────────┴──────────────────────────┐
│                crates/core (business)               │
│  model/      — доменные сущности (VaultDoc, Store,  │
│                Transaction, Settings, Chain)        │
│  appconfig.rs— настройки приложения (вне vault, JSON)│
│  service.rs  — UnlockedVault: все мутации + аналитика│
│  pricing/    — ценовые провайдеры, кэш, FX           │
│  crypto/     — XChaCha20-Poly1305 + Argon2id,        │
│                фиксированный 55-байт header (AAD)    │
│  storage/    — SnapshotFormat (JSON/MessagePack),    │
│                атомарная запись                      │
│  market/     — OHLCV-свечи Binance для графиков     │
│  ipc_contract.rs — DTO команд (ts-rs → ipc.gen.d.ts)  │
└─────────────────────────────────────────────────────┘
```

## Где что лежит на диске

| Путь (Linux) | Что | Формат |
|---|---|---|
| `~/.local/share/dev.mwt.desktop/vault.wallet` | хранилище: настройки vault, магазины, операции, **кэш цен и FX** | зашифрован (JSON в dev, MessagePack в release; старый bincode читается) |
| `~/.config/dev.mwt.desktop/app_settings.json` | тема, язык, базовая валюта, избранные валюты | обычный JSON |
| `~/.config/dev.mwt.desktop/currencies.json` | кэш каталога валют (фиат + крипто) | обычный JSON |
| `~/.config/dev.mwt.desktop/recent_vaults.json` | недавние хранилища | обычный JSON |

Кэш цен и FX **внутри vault** (`VaultDoc.price_cache` / `fx_cache`) — он
зашифрован и переживает рестарт, но привязан к конкретному хранилищу, у каждого
свой. Каталог валют — в конфигах приложения: он не секретный и одинаков для всех
vault.

## Поток данных

1. Пользователь вводит мастер-пароль → `auth::unlock_vault` →
   `VaultFile::open(path, password)` → `crypto::open_bytes` (AEAD-проверка
   пароля) → десериализация `VaultDoc`. Результат кладётся в `AppState.vault`.
2. Все мутации (store/tx/settings) идут через `UnlockedVault::*` методы,
   каждый из которых в конце вызывает `persist()` — пере-шифрование и
   атомарная запись на диск.
3. Цены: `refresh_all` сначала снимает снимок нужного (активы, базовая валюта)
   и отпускает мьютекс, затем делает сетевые фетчи (библиотечный await),
   затем снова захватывает vault и применяет кэш через `apply_prices/apply_fx`.
4. Frontend слушает событие `prices-updated` и инвалидирует TanStack-кэш.
5. Настройки пользователя не идут через vault: `App.vue` читает
   `get_app_settings` до разблокировки, поэтому тема и язык применяются уже на
   экране входа; базовая валюта после `open`/`create` заталкивается в vault
   через `sync_base_currency`.

## Принципы

- **Ядро без GUI.** Всё тестируемое — в core, тесты запускаются без Tauri.
- **Безопасность по умолчанию.** Один пароль, никаких хранимых верификаторов;
  целостность обеспечивает AEAD (header = AAD, любые изменения фейлят чек).
- **Оффлайн-френдли.** Прайсы/FX копят историю в vault (`price_cache`,
  `fx_cache`); без сети используется последний известный курс, а операции
  оцениваются по курсу своей даты.
- **Деньги = Decimal.** `Amount` — newtype над `rust_decimal::Decimal`,
  всегда сериализуется строкой (точность + стабильность JSON и MessagePack).
- **Настройки, которые описывают пользователя, живут вне vault.** Тема, язык,
  базовая валюта и избранные валюты не шифруются и не дублируются в каждом
  хранилище; им нечего прятать, зато второй vault должен унаследовать их, а
  экран входа — отрисоваться на нужном языке до ввода пароля. Vault-scoped
  настройки (интервалы, трей, виджеты) остаются в документе.
- **Форматы снапшота.** Dev — человекочитаемый JSON, release — MessagePack с
  именами полей (до v2 — bincode, он только читается). Тип хранится в header
  файла (`payload_fmt`), файл самодостаточен. Старые версии схемы мигрируют
  при открытии, см. `docs/data-model.md` → «Версии и миграции».

## Масштабирование

Пока монолит. Если ядро вырастет — потенциальные слоты: вынос тяжёлых
численных задач в отдельный фон-процесс, плагины провайдеров данных через
feature-флаги в `trade-core`. IPC-контракт остаётся источником истины для FE.