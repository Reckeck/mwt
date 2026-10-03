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
│             pricing,portfolio,settings,sim}         │
└──────────────────────────┬──────────────────────────┘
                           │ (общая зависимость через workspace)
┌──────────────────────────┴──────────────────────────┐
│                crates/core (business)               │
│  model/      — доменные сущности (VaultDoc, Store,  │
│                Transaction, Settings, Chain, Sim)   │
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

## Принципы

- **Ядро без GUI.** Всё тестируемое — в core, тесты запускаются без Tauri.
- **Безопасность по умолчанию.** Один пароль, никаких хранимых верификаторов;
  целостность обеспечивает AEAD (header = AAD, любые изменения фейлят чек).
- **Оффлайн-френдли.** Прайсы/FX кэшируются в vault (`price_cache`,
  `fx_cache`) с TTL; без сети используется последний известный курс.
- **Деньги = Decimal.** `Amount` — newtype над `rust_decimal::Decimal`,
  всегда сериализуется строкой (точность + JSON/bincode стабильность).
- **Два формата снапшота.** Dev — человекочитаемый JSON, release — компактный
  bincode. Тип хранится в header файла (`payload_fmt`), файл самодостаточен.

## Масштабирование

Пока монолит. Если ядро вырастет — потенциальные слоты: вынос симулятора в
отдельный фон-процесс (он CPU-интенсивный), плагины провайдеров данных через
feature-флаги в `trade-core`. IPC-контракт остаётся источником истины для FE.