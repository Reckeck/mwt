# Архитектура

## Обзор

TradeApp — это Tauri 2 desktop-приложение с монолитным Rust-ядром и Vue-фронтендом,
связка через IPC (invoke + events). Вся бизнес-логика живёт в `crates/core`
(чистый Rust, ноль зависимостей от Tauri) и покрыта юнит-тестами; `crates/app`
представляет собой тонкую оболочку: `tauri::Builder`, команды и состояние.

```
┌─────────────────────────────────────────────────────┐
│                    Vue 3 frontend                    │
│  Pinia (состояние) · TanStack Query (сеть+кэш)      │
│  shadcn-vue (UI) · vue-echarts (графики)            │
│  vue-i18n (ru/en) · types из src/lib/ipc.gen.d.ts        │
└──────────────────────────┬──────────────────────────┘
                           │ invoke / events
┌──────────────────────────┴──────────────────────────┐
│                crates/app (Tauri shell)             │
│  AppState { vault: Mutex<Option<UnlockedVault>>,    │
│             vault_path: Mutex<PathBuf>, pricing }   │
│  commands/{auth,stores,transactions,categories,     │
│             pricing,portfolio,settings,sim,         │
│             app_settings,vault_recents}             │
└──────────────────────────┬──────────────────────────┘
                           │ (общая зависимость через workspace)
┌──────────────────────────┴──────────────────────────┐
│                crates/core (business)               │
│  model/      — доменные сущности (VaultDoc, Store,  │
│                Transaction, Settings, Chain, Sim)   │
│  appconfig.rs— настройки приложения (вне vault, JSON)│
│  service.rs  — UnlockedVault: все мутации + аналитика│
│  pricing/    — ценовые провайдеры, кэш, FX           │
│  crypto/     — XChaCha20-Poly1305 + Argon2id,        │
│                фиксированный 55-байт header (AAD)    │
│  storage/    — SnapshotFormat (JSON/bincode),        │
│                атомарная запись                      │
│  sim/        — стратегии, индикаторы, engine реплея  │
│  ipc_contract.rs — DTO команд (ts-rs → ipc.gen.d.ts)  │
└─────────────────────────────────────────────────────┘
```

## Где что лежит на диске

| Путь (Linux) | Что | Формат |
|---|---|---|
| `~/.local/share/dev.tradeapp.desktop/vault.wallet` | хранилище: настройки vault, магазины, операции, **кэш цен и FX** | зашифрован (JSON в dev, bincode в release) |
| `~/.config/dev.tradeapp.desktop/app_settings.json` | тема, язык, базовая валюта, избранные валюты | обычный JSON |
| `~/.config/dev.tradeapp.desktop/currencies.json` | кэш каталога валют (фиат + крипто) | обычный JSON |
| `~/.config/dev.tradeapp.desktop/recent_vaults.json` | недавние хранилища | обычный JSON |

Кэш цен и FX **внутри vault** (`VaultDoc.price_cache` / `fx_cache`) — он
зашифрован и переживает рестарт, но привязан к конкретному хранилищу, у каждого
свой. Каталог валют — в конфигах приложения: он не секретный и одинаков для всех
vault.

## Поток данных

1. Пользователь вводит мастер-пароль → `auth::unlock_vault` →
   `VaultFile::open(path, password)` → `crypto::open_bytes` (AEAD-проверка
   пароля) → десериализация `VaultDoc`. Результат кладётся в `AppState.vault`.
2. Все мутации (store/tx/settings/sim) идут через `UnlockedVault::*` методы,
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
- **Оффлайн-френдли.** Прайсы/FX кэшируются в vault (`price_cache`,
  `fx_cache`) с TTL; без сети используется последний известный курс.
- **Деньги = Decimal.** `Amount` — newtype над `rust_decimal::Decimal`,
  всегда сериализуется строкой (точность + JSON/bincode стабильность).
- **Настройки, которые описывают пользователя, живут вне vault.** Тема, язык,
  базовая валюта и избранные валюты не шифруются и не дублируются в каждом
  хранилище; им нечего прятать, зато второй vault должен унаследовать их, а
  экран входа — отрисоваться на нужном языке до ввода пароля. Vault-scoped
  настройки (интервалы, трей, виджеты) остаются в документе.
- **Два формата снапшота.** Dev — человекочитаемый JSON, release — компактный
  bincode. Тип хранится в header файла (`payload_fmt`), файл самодостаточен.

## Масштабирование

Пока монолит. Если ядро вырастет — потенциальные слоты: вынос симулятора в
отдельный фон-процесс (он CPU-интенсивный), плагины провайдеров данных через
feature-флаги в `trade-core`. IPC-контракт остаётся источником истины для FE.