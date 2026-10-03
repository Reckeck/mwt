# AGENTS.md — инструкции для ИИ-разработчиков

Рабочий язык проекта: **русский** для пользовательских текстов и документации,
**английский** для кода (имена, комментарии в коде).

## Команды (mise)

Всегда используй `mise run …` (task-файл `mise.toml`), не хардкодь версии:

| Команда | Что делает |
|---|---|
| `mise run test` | `cargo test --workspace` |
| `mise run typecheck` | `vue-tsc --noEmit` |
| `mise run lint` | `pnpm exec eslint .` |
| `mise run gen-types` | пересоздать `src/lib/ipc.gen.d.ts` из Rust (ts-rs) |
| `mise run check` | lint + typecheck + tests |
| `mise run build` | тесты + typecheck + `pnpm tauri build` (deb/rpm/AppImage) |
| `mise run dev` | Tauri dev (`pnpm tauri dev`; Tauri v2 CLI сам находит `crates/app/tauri.conf.json` из корня воркспейса) |
| `pnpm dev` | Vite-фронтенд без Tauri |

Напрямую полезно: `cargo build --workspace`, `cargo test -p trade-core`.

## Структура

```
Cargo.toml            # cargo workspace: crates/core + crates/app, edition 2024
crates/core/          # весь бизнес-логика, БЕЗ Tauri
  src/lib.rs          # re-export: Amount, Error/Result, model::*, VaultFile, SnapshotFormat
  src/error.rs        # Error enum: Crypto/Secret/NotFound/Unsupported/…, метод .code()
  src/amount.rs       # Amount (Decimal newtype), сериализация строкой; кастомный ts-rs TS impl
  src/model/          # домен: chain.rs, store.rs, transaction.rs, vault.rs (VaultDoc, Settings…)
  src/crypto/         # header.rs (55 байт, AAD) + mod.rs (seal/open/atomic_write)
  src/storage/        # SnapshotFormat (Json/Bin + for_profile()), VaultFile::create/open/save
  src/pricing/        # PricingService: fetch_prices/fetch_fx (чистые, без мутации doc)
  src/service.rs      # UnlockedVault: мутации, холдинги, портфель, история, PnL
  src/sim/            # strategy model + engine.rs (replay, индикаторы, GBM)
  src/ipc_contract.rs # DTO для команд (входы/выходы IPC)
  src/bin/gen_types.rs# генератор src/lib/ipc.gen.d.ts (ts-rs export_all_to + слияние)
crates/app/           # Tauri shell: builder, commands/*, tauri.conf.json, capabilities/
  src/commands/       # auth, stores, transactions, categories, pricing, portfolio, calc, settings, sim, exchange
  src/lib.rs          # tauri::Builder + generate_handler![…] (список ВСЕХ команд), AppState (vault: Mutex<Option<UnlockedVault>>, vault_path)
src/                  # фронтенд (Vue 3 + Pinia + TanStack Query + vue-i18n + ECharts)
  views/              # экраны, по одному .vue на маршрут src/router/index.ts
  components/ui/      # примитивы shadcn-vue (Reka UI), имена в нижнем регистре
  components/wallet/  # компоненты кошелька; ipc вызывают напрямую через lib/ipc.ts
  stores/             # Pinia: vault.ts, settings.ts, tradeLookup.ts
  lib/ipc.ts          # ЕДИНСТВЕННАЯ точка вызова Tauri (call/onEvent) + мок для pnpm dev в браузере
  lib/ipc.gen.d.ts    # AUTO-GENERATED, пересоздаётся mise run gen-types
  i18n/locales/       # ru.json + en.json, плоские точечные ключи ("nav.dashboard"); набор ключей должен совпадать
  features/           # 8 пустых каталогов-заглушек (auth, charts, dashboard, pricing, settings, sim, stores, transactions). Кода нет, git их не трекает. Реальный код — в views/ и components/
.opencode/skills/     # project-specific skills (см. таблицу ниже)
mise.toml             # tools + env + tasks
docs/                 # architecture, data-model, ipc, security, roadmap
```

## Правила работы с кодом

1. **Core не зависит от Tauri.** Всё, что можно протестировать юнит-тестами
   (крипта, модель, симулятор), тестируется в core. Команды в `crates/app` —
   только тонкая IPC-обёртка.
2. **Деньги — только `Amount` (Decimal).** Не используй `f64` в доменных
   количествах и ценах. `f64` допустим только внутри engine симулятора
   (и помечай знающими комментарием).
3. **Асинхронные команды Tauri.** Никогда не держи
   `MutexGuard<Option<UnlockedVault>>` через `.await` (future не `Send`).
   Сначала скопируй нужные данные, отпусти guard, потом await, потом снова
   захвати. Образец: `refresh_all` в `commands/pricing.rs` и `run_strategy`
   в `commands/sim.rs`.
4. **Сеть от кэша отделена.** `PricingService::fetch_prices/fetch_fx` —
   чистые (не трогают `doc`), `apply_prices/apply_fx` — применяют результат.
5. **Смена схемы vault-файла** = бамп `VAULT_FORMAT_VERSION`, миграция в
   `service.rs`, и тест на открытие старого файла.
6. **IP-контракт.** Держи `#[derive(TS)]` у всех типов из `ipc_contract.rs`
   и доменных структур, что попадают в команды. После изменения Регенерируй:
   `mise run gen-types`. Файл `src/lib/ipc.gen.d.ts` руками не править.
   Имя `.gen.d.ts` выбрано намеренно: TypeScript не включает в программу
   `foo.d.ts`, если рядом лежит `foo.ts` (база имени «затеняется»), поэтому
   файл называется `ipc.gen.d.ts` рядом с `ipc.ts`.
7. **Edition 2024**: `gen`/`await` зарезервированы — нельзя `rng.gen()`
   (используй `rand::random::<f64>()`), нельзя имена методов/переменных `gen`.
8. **Вектор тестирования крипты**: wrong password/tampered file должны давать
   `Error::Secret`, никогда паниковать.

## Конвенции

- Код на английском; комменты обязательными не делаем, но где нужен контекст —
  пишем.
- Форматирование: `cargo fmt`. Линт: `cargo clippy --workspace -- -D warnings`
  (профильный, в CI-стиле).
- Никаких секретов в репозитории; `.gitignore` игнорит `data/`, `*.wallet`,
  `.rust-target/`, `node_modules/`, `dist/`, `src-tauri/target/`.
- Никаких секретов в репозитории; `.gitignore` игнорит `data/`, `*.wallet`,
  `.rust-target/`, `node_modules/`, `dist/`, `src-tauri/target/`.

## Проверки и коммиты (Jujutsu)

Репозиторий под **jj** (colocated с git). Рабочая копия автоматически
снапшится в change `@` — `git add` не существует и не нужен.

### Гейт перед коммитом

```bash
cargo fmt             # сначала форматирование, потом проверки
mise run check        # eslint + vue-tsc --noEmit + cargo test --workspace
```

Гейт зелёный → коммитим. Не зелёный → коммит запрещён, сначала чини.
Никогда не «фиксим» падение теста удалением или ослаблением проверки.

Быстрее, когда правка заведомо локальная:

| Изменения | Команда |
|---|---|
| `src/**` (фронтенд) | `mise run lint && mise run typecheck` |
| `crates/core/**`, `crates/app/**` | `mise run test` |
| DTO, модели, `ipc_contract.rs` | `mise run test && mise run gen-types` — обязательно диффай `src/lib/ipc.gen.d.ts`, он должен измениться |
| финальный, перед коммитом | `mise run check` |

### Коммит

```bash
jj status        # что попадёт в change @ (включая незакоммиченное — jj уже учёл)
jj diff          # что именно изменилось
jj describe -m "fix: ..."   # описать текущий change
jj new           # следующий change поверх текущего
```

`jj commit` = `describe` + `new`. Одно изменение — один change; не смешивай
рефакторинг с фичей в одном сообщении.

**Сообщение:** conventional commits, **на английском**, тема ≤ 72 символов.
Типы: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`. Тело — если
изменение неочевидное: *что* изменилось и *почему*, а не *какие файлы тронул*
(это видно в diff).

**Запрещено:** `git commit`, `git add`, `git stash` (снапшот jj делает сам),
переписывать change, который уже ушёл на сервер (`jj git push`).

Коммит — **только по явной просьбе пользователя**.

Нюансы (split, squash, bookmark вместо веток, rebase, undo, работа с
незакоммиченным состоянием) — в скилле `jj-workflow`
(`.opencode/skills/jj-workflow/SKILL.md`). Коммиты непустого рабочего
состояния влекут за собой мусор в истории — сначала посмотри `jj diff`.

## Project-specific skills

Лежат в `.opencode/skills/<name>/SKILL.md`, грузятся по необходимости.
Загружай нужный **до** правки кода, а не после:

| Skill | Когда |
|---|---|
| `jj-workflow` | коммиты, проверки, ветки, отмена изменений |
| `ipc-command` | новая или изменённая Tauri-команда (DTO → core → хендлер → gen-types) |
| `vault-crypto` | формат vault, миграции, крипта, `Amount`, секреты |
| `frontend-feature` | Vue-фича, i18n, TanStack Query, графики |
| `sim-strategy` | симулятор, стратегии, GBM, индикаторы |
| `pricing-offline` | цены/FX, провайдеры, кэш, офлайн-деградация |

## Definition of Done

- [ ] `cargo fmt` выполнен, диффа от автоформатирования нет
- [ ] `mise run check` — зелёные (eslint + vue-tsc + `cargo test --workspace`)
- [ ] `mise run gen-types` — `src/lib/ipc.gen.d.ts` актуален (если менялись DTO)
- [ ] Новые DTO в `ipc_contract.rs` покрыты ts-rs и командой в `crates/app`
- [ ] Команда зарегистрирована в `generate_handler!` и в таблице `docs/ipc.md`
- [ ] UI-тексты добавлены в оба локаля (`src/i18n/locales/{ru,en}.json`)
- [ ] `jj diff` содержит только то, что относится к задаче
- [ ] Коммит сделан **только если пользователь попросил**, гейт до этого зелёный