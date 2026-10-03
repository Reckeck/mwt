# Роадмап

Шкала статусов: `[x]` сделано · `[~]` в работе · `[ ]` запланировано.

## Фаза 1 — Фундамент `[~]`
- `[x]` Cargo workspace (`crates/core` + `crates/app`), mise-окружение, задачи
- `[x]` Domain-модель (VaultDoc, Store/Holding, Transaction, Settings, Chain, Sim)
- `[x]` Крипто: Argon2id + XChaCha20-Poly1305, header=AAD, атомарная запись
- `[x]` Хранилище: JSON (dev) / bincode (release), VaultFile::create/open/save
- `[x]` UnlockedVault: мутации, holdings (derived), сводка, история, realized PnL
- `[x]` Прайсы+FX: CoinGecko→CoinCap→Binance, open.er-api.com, кэш TTL
- `[x]` IPC-контракт + генератор `src/lib/ipc.gen.d.ts` (ts-rs)
- `[x]` Tauri shell: builder, AppState, команды (auth/stores/…/sim)
- `[x]` Документация (README, AGENTS, docs/*)
- `[~]` Фронтенд-скаффолд (см. Фазу 2)
- `[ ]` Первый релиз-сборок-пайплайн: bundle deb/rpm/AppImage (финальная верификация)

## Фаза 2 — Frontend `[x]`
- `[x]` Скаффолд Vue 3.5 + Vite + TS, shadcn-vue + Tailwind v4, vue-i18n (ru/en), Pinia, TanStack Query, vue-echarts
- `[x]` Экран Unlock / Create Vault (пароль, TTL авто-лок, индикатор аутентификации)
- `[x]` Dashboard: сводка портфеля, by-chain/by-store, FX-курс, «цена обновлена»
- `[x]` Stores: CRUD, архив, цвет, переход к holdings
- `[x]` Transactions: список, фильтры, добавление income/expense/transfer/trade, комментарии, категории
- `[x]` Adder-формы с autocomplete активов (`search_assets`), выбор сети + tx-hash (explorer-ссылка)
- `[x]` Charts: динамика портфеля, распределение по активам/сетям (ECharts)
- `[x]` Settings: базовая валюта, тема light/dark/system, локаль, decimals, смена пароля
- `[x]` Strategy Lab: конструктор правил (формы + YAML-превью), прогон real/synthetic, equity-кривая и метрики, бумажный аккаунт
- `[x]` **Wallet page**: сайдбар с кошельками, калькулятор (конвертер + PnL), формы транзакций, позиции, история, график навигации

## Фаза 3 — Умный рост `[ ]`
- `[ ]` Портфельный отчёт: за период (прибыль/убыток, дивиденды/стекинг)
- `[ ]` Category-аналитика и бюджеты по категориям
- `[ ]` Импорт/экспорт транзакций (CSV), шаблоны
- `[ ]` Снапшот-бэкап и восстановление vault с проверкой целостности
- `[ ]` Телеграм/веб-хук для цены и сигналов (пул-модель, без ключей в vault-репо)

## Фаза 4 — Оборона / polish `[ ]`
- `[ ]` Auto-lock по неактивности, пауза при N неудачных попытках unlock
- `[ ]` Windows/macOS сборки в CI (GitHub Actions + mise-шаги идентичны локальным)
- `[ ]` E2E smoke: создать vault → транзакция → график → lock/unlock
- `[ ]` Производительность: индексы по transactions, ленивые графики на big data
- `[ ]` Updater (tauri-plugin-updater), подпись артефактов

## Не-скоуп (сознательно)
- Хранение ключей в TPM/YubiKey (v1 — пароль; см. docs/security.md)
- Зеро-знание-синк между устройствами (файл — источник истины)
- Торговые ордера на биржах (учёт — да, исполнение — нет)