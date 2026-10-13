# Multi Wallet Tracker (MWT)

Зашифрованный десктоп-менеджер финансового и крипто-портфеля.
Один пароль → один зашифрованный файл-хранилище (vault). Всё остальное внутри.

## Возможности

- **Vault** — единый файл (по умолчанию XDG AppData/AppData `vault.wallet`),
  зашифрован XChaCha20-Poly1305, ключ — Argon2id от мастер-пароля.
  Ошибочный пароль = криптографический отказ, без хранимого верификатора.
- **Хранилища (Stores)** — счета/кошельки-«категории» внутри vault
  (наличные, биржа, hot/cold storage, банк).
- **Транзакции** — доходы/расходы, переводы между хранилищами, трейды
  (leg in + leg out), комментарий у каждой, привязка сети и tx-hash.
- **Прайсы и FX** — бесплатная связка CoinGecko → CoinCap → Binance
  для крипты и open.er-api.com для валют, кэш с TTL и offline-режимом.
- **Графики** — Apache ECharts через vue-echarts.
- **i18n** — русский и английский (vue-i18n) с мгновенным переключением без
  перезагрузки, темы light/dark/system.
- **Подсказки по сделкам** — введённый `tx_hash` best-effort обогащается данными
  из публичных обозревателей (Bitcoin, Solana); неоднозначные хеши сети
  (`0x…`) требуют ручного выбора, а не догадки.
- **Импорт сделок Bybit** — read-only ключ в зашифрованном vault, список последних
  исполнений (`spot` / `linear` / `inverse`) для предзаполнения формы трейда.
  Приложение не умеет выставлять ордера: в клиенте есть только GET-эндпоинты.
  Без ключа или без сети форма просто остаётся заполненной вручную.

## Стек

| Слой | Технологии |
|---|---|
| Shell | Tauri 2 (Rust), плагины opener/fs/dialog |
| Core | Чистый Rust (cargo workspace `crates/core`), без зависимости от Tauri |
| Хранение | JSON (dev) / MessagePack (release), версии с миграцией, атомарная запись (tmp + fsync + rename) |
| FE | Vue 3.5 + Vite 8 + TypeScript, Pinia, TanStack Query, shadcn-vue (Reka UI) + Tailwind v4, vue-echarts, vue-i18n |
| Toolchain | mise (node/pnpm/rust), команды см. `mise run` |

## Быстрый старт

```bash
mise install          # установить node/pnpm/rust по mise.toml
mise run setup        # pnpm install && cargo fetch
mise run dev          # Tauri dev-сборка (frontend + core)
mise run test         # cargo test --workspace
mise run typecheck    # vue-tsc
mise run lint         # biome (линт + формат)
mise run gen-types    # пересоздать src/shared/api/ipc.gen.d.ts из Rust (ts-rs)
mise run check        # всё вышеперечисленное
mise run build        # release-бандл (deb/rpm/AppImage)
```

Подробнее для разработчиков — в [AGENTS.md](AGENTS.md).
Архитектура, модель данных, безопасность, IPC и роадмап — в [`docs/`](docs/).

## Состояние проекта

- Backend (`trade-core` + `trade-app` shell): собирается, 66 юнит-тестов зелёные,
  DTO-типы генерируются в `src/shared/api/ipc.gen.d.ts`.
- Frontend: MVP готово — Dashboard, Stores CRUD, Transactions (income/expense/transfer/trade),
  Charts, Settings, **Wallet page** (сайдбар кошельков, калькулятор конвертер+PnL,
  формы транзакций, позиции, история, график навигации). Формы операций, калькулятор
  и создание кошелька открываются модальными окнами.
- Обогащение сделок: инспекция `tx_hash` и read-only импорт последних сделок Bybit
  подключены к форме трейда; ключ хранится в vault, схема документа — версия 1
  (автоматическая миграция с версии 1).

## Лицензия

Проприетарная (лицензия кодируется позже). Все права зарезервированы.