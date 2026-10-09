# AGENTS.md — инструкции для ИИ-разработчиков

Рабочий язык проекта: **русский** для пользовательских текстов и документации,
**английский** для кода (имена, комментарии в коде).

## Команды (mise)

Всегда используй `mise run …` (task-файл `mise.toml`), не хардкодь версии:

| Команда | Что делает |
|---|---|
| `mise run test` | `cargo test --workspace` |
| `mise run typecheck` | `vue-tsc --noEmit` |
| `mise run lint` | `pnpm exec biome check .` (линт + формат, Biome) |
| `mise run format` | `pnpm exec biome check --write .` — автофикс Biome |
| `mise run clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `mise run gen-types` | пересоздать `src/shared/api/ipc.gen.d.ts` из Rust (ts-rs) |
| `mise run check` | lint + typecheck + clippy + tests |
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
  src/appconfig.rs    # настройки приложения ВНЕ vault: theme, locale, base_currency, favorites
  src/crypto/         # header.rs (55 байт, AAD) + mod.rs (seal/open/atomic_write)
  src/storage/        # SnapshotFormat (Json/Bin + for_profile()), VaultFile::create/open/save
  src/pricing/        # PricingService: fetch_prices/fetch_fx (чистые, без мутации doc)
  src/service.rs      # UnlockedVault: мутации, холдинги, портфель, история, PnL
  src/market/         # OHLCV-свечи Binance для графиков (жило в sim/, лаборатория удалена)
  src/ipc_contract.rs # DTO для команд (входы/выходы IPC)
  src/bin/gen_types.rs# генератор src/shared/api/ipc.gen.d.ts (ts-rs export_all_to + слияние)
crates/app/           # Tauri shell: builder, commands/*, tauri.conf.json, capabilities/
  src/commands/       # auth, stores, transactions, categories, pricing, portfolio, calc, settings, exchange
  src/lib.rs          # tauri::Builder + generate_handler![…] (список ВСЕХ команд), AppState (vault: Mutex<Option<UnlockedVault>>, vault_path)
src/                  # фронтенд (Vue 3 + Pinia + TanStack Query + vue-i18n + ECharts), FSD v2.1
  main.ts             # bootstrap: createApp + роутер/пиния/vue-query/i18n
  app/                # верхний слой: точка входа и инфраструктура
    providers/        # router/, query/ — инфраструктурные провайдеры
    styles.css        # глобальные стили (Tailwind v4, base)
  pages/              # экраны, по одному на маршрут app/providers/router; ТОЛЬКО композиция
    dashboard/ stores/ wallet/ transactions/ charts/ trading/ settings/ unlock/ app-shell/
  widgets/            # композитные блоки страниц (store-header, dashboard-total, sidebar-wallets, …)
  features/           # пользовательские действия (vault-unlock, store-create, price-refresh, …)
  entities/           # бизнес-сущности: store/, transaction/, asset/, currency/
  shared/             # нижний слой, ничего не импортирует выше себя
    ui/               # примитивы shadcn-vue/Reka: ПАПКА на компонент + index.ts
    lib/              # утилиты: format, echarts, theme, locale, transactions, vault-error…
    api/ipc.ts        # ЕДИНСТВЕННАЯ точка вызова Tauri (call/onEvent) + мок для pnpm dev
    api/ipc.gen.d.ts  # AUTO-GENERATED, пересоздаётся mise run gen-types
    store/            # Pinia: vault.ts, settings.ts (глобальное состояние)
    config/i18n/locales/  # ru.json + en.json, плоские точечные ключи; набор ключей должен совпадать
.agents/skills/       # скиллы агентов, ВСЕ здесь (см. таблицу ниже)
mise.toml             # tools + env + tasks
docs/                 # architecture, data-model, ipc, security, roadmap, ui-design, json-editor
```

## Правила работы с кодом

1. **Core не зависит от Tauri.** Всё, что можно протестировать юнит-тестами
   (крипта, модель, сервис), тестируется в core. Команды в `crates/app` —
   только тонкая IPC-обёртка.
2. **Деньги — только `Amount` (Decimal).** Не используй `f64` в доменных
   количествах и ценах. `f64` допустим только в `market/` — OHLCV-свечи
   для графиков, а не деньги.
3. **Асинхронные команды Tauri.** Никогда не держи
   `MutexGuard<Option<UnlockedVault>>` через `.await` (future не `Send`).
   Сначала скопируй нужные данные, отпусти guard, потом await, потом снова
   захвати. Образец: `refresh_all` в `commands/pricing.rs`.
4. **Сеть от кэша отделена.** `PricingService::fetch_prices/fetch_fx` —
   чистые (не трогают `doc`), `apply_prices/apply_fx` — применяют результат.
5. **Схема vault-файла меняется БЕЗ бампа версии.** `VAULT_FORMAT_VERSION`
   зафиксирован на `1`, миграций нет, а `storage::decode` отвергает любую
   другую версию. Аддитивные поля добавляй с `#[serde(default)]`; удалённые
   поля serde молча игнорирует. **Не бампай `VAULT_FORMAT_VERSION` без
   явного разрешения пользователя** — прошлый бамп молча закрывал доступ к
   собственному vault, и это решение осознанное.
6. **IP-контракт.** Держи `#[derive(TS)]` у всех типов из `ipc_contract.rs`
   и доменных структур, что попадают в команды. После изменения Регенерируй:
   `mise run gen-types`. Файл `src/shared/api/ipc.gen.d.ts` руками не править.
   Имя `.gen.d.ts` выбрано намеренно: TypeScript не включает в программу
   `foo.d.ts`, если рядом лежит `foo.ts` (база имени «затеняется»), поэтому
   файл называется `ipc.gen.d.ts` рядом с `ipc.ts`.
7. **Edition 2024**: `gen`/`await` зарезервированы — нельзя `rng.gen()`
   (используй `rand::random::<f64>()`), нельзя имена методов/переменных `gen`.
8. **Вектор тестирования крипты**: wrong password/tampered file должны давать
   `Error::Secret`, никогда паниковать.
9. **Границы слоёв FSD.** `app → pages → widgets → features → entities → shared`.
   Слой импортирует только из себя и из перечисленных ниже; `shared` не
   импортирует ничего из остальных слоёв. Соседние слайсы одного слоя
   импортируют только через public API (`index.ts` слайса). Нарушение границы
   — не стилистика, а повод вынести общее в `shared`.
10. **Feature = пользовательское действие, widget = композит секции, entity =
    представление сущности.** Если блок только показывает сущность — это
    `entities/<name>/ui`; если собирает секцию страницы из нескольких
    сущностей/фич — `widgets`; если выполняет операцию пользователя — `features`.
    Страница в `pages` — тонкая композиция, без собственной доменной логики.
11. **UI-компонент — папка с `index.ts`.** В `shared/ui` и `entities/*/ui`
    каждый компонент лежит в своей директории, а `index.ts` **обязательно**
    экспортирует: именованные компоненты (без `default`-экспорта снаружи),
    типы пропсов и константы (варианты, размеры, списки опций). Типы и
    константы, нужные наружу, живут в `*.types.ts` / `*.constants.ts` внутри
    папки — в `<script setup>` их экспортировать нельзя.

## Конвенции

- **Слои FSD.** Структура и правила импортов описаны в разделе «Правила
  работы с кодом» (пункты 9–11). Кратко: `app/providers` (роутер, query),
  `pages` (композиция), `widgets` (блоки страниц), `features` (действия),
  `entities` (сущности), `shared` (ui, lib, api, store, config).

- Код на английском; комменты обязательными не делаем, но где нужен контекст —
  пишем.
- Форматирование и линтRust: `cargo fmt` + `mise run clippy`
  (`cargo clippy --workspace --all-targets -- -D warnings`, профильный, в CI-стиле).
- Форматирование и линт фронтенда: Biome (`biome.json`) — `mise run lint`
  проверяет, `mise run format` чинит. Настройки под текущий стиль кода:
  одинарные кавычки, без точек с запятой, 2 пробела, ширина 120.
  ESLint в проекте больше нет.
- **Никаких однобуквенных имён.** Rust: линт `clippy::min_ident_chars`
  (порог 1, список исключений пустой — `clippy.toml` + `[workspace.lints]`),
  так что `i`, `j`, `x` тоже запрещены; неиспользуемую переменную
  помечай `_`-префиксом. Фронтенд: `style/useNamingConvention` в `biome.json`
  требует минимум 2 символа (regex `t|.{2,}`) у переменных и параметров;
  единственное исключение — `t` из `const { t } = useI18n()` (деструктуризация
  правилом и так не проверяется, а имя `t` разрешено явно).
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
mise run check        # biome check + vue-tsc --noEmit + clippy + cargo test --workspace
```

Гейт зелёный → коммитим. Не зелёный → коммит запрещён, сначала чини.
Никогда не «фиксим» падение теста удалением или ослаблением проверки.

Быстрее, когда правка заведомо локальная:

| Изменения | Команда |
|---|---|
| `src/**` (фронтенд) | `mise run lint && mise run typecheck` |
| `crates/core/**`, `crates/app/**` | `mise run test && mise run clippy` |
| DTO, модели, `ipc_contract.rs` | `mise run test && mise run gen-types` — обязательно диффай `src/shared/api/ipc.gen.d.ts`, он должен измениться |
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
изменение неочевидно: *что* изменилось и *почему*, а не *какие файлы тронул*
(это видно в diff).

### Соавтор: агент указывает себя сам

Каждый коммит, сделанный агентом, несёт трейлер с **именем инструмента**,
которым агент работает:

```
Co-authored-by: <Инструмент> <email>
```

- jj его **не** подставляет: `templates.commit_trailers` в repo-конфиге
  выключен намеренно, иначе любой агент подписывался бы чужим именем.
- Агент дописывает строку **сам**, последним абзацем сообщения, ровно одну
  на инструмент; чужие трейлеры при переписывании сообщения не убирает.
- Таблица имён и порядок проверки — в скилле `commit-coauthor`.

**Запрещено:** `git commit`, `git add`, `git stash` (снапшот jj делает сам),
переписывать change, который уже ушёл на сервер (`jj git push`).

Коммит — **только по явной просьбе пользователя**.

Нюансы (split, squash, bookmark вместо веток, rebase, undo, работа с
незакоммиченным состоянием) — в скилле `jj-workflow`
(`.agents/skills/jj-workflow/SKILL.md`). Коммиты непустого рабочего
состояния влекут за собой мусор в истории — сначала посмотри `jj diff`.

## Skills агентов

**Все скиллы лежат в `.agents/skills/<name>/SKILL.md`** — это единственное
место. Каталога `.opencode/skills/` больше нет. Загружай нужный **до** правки
кода, а не после:

| Skill | Когда |
|---|---|
| `jj-workflow` | коммиты, проверки, ветки, отмена изменений |
| `commit-coauthor` | трейлер `Co-authored-by` с именем своего инструмента в каждом коммите |
| `feature-sliced-design` | разработка frontend/ui |
| `ipc-command` | новая или изменённая Tauri-команда (DTO → core → хендлер → gen-types) |
| `vault-crypto` | формат vault, миграции, крипта, `Amount`, секреты |
| `frontend-feature` | Vue-фича, i18n, TanStack Query, графики |
| `ui-design-system` | вёрстка, токены отступов, формы, **уведомления** |
| `shadcn-vue` | любой shadcn-компонент: добавить, найти, починить, скомпоновать |
| `json-editor` | редактор vault в формате JSON (Настройки → Просмотр JSON) |
| `pricing-offline` | цены/FX, провайдеры, кэш, офлайн-деградация |

### Управление скиллами

Скиллы ставятся и обновляются утилитой `skills` (npm-пакет `skills`,
запускается через пакетный раннер проекта). Каталог один — `.agents/skills/`,
его читают OpenCode, Claude Code, Codex и Zed.

```bash
pnpm dlx skills add <registry> --agent '*' -y   # поставить/обновить скилл
pnpm dlx skills list                             # что установлено и кем читается
```

- `--agent '*' -y` обязательны: без них CLI требует TTY и умирает в агентском
  окружении («Interactive prompt required but stdin is not a TTY»).
- Скилл из реестра (`Source: <repo>`) обновляй только этой командой, руками не
  правь — апдейт приедет поверх.
- Скилл `Source: local` написан руками в `.agents/skills/` и обновляется
  вместе с кодом проекта.
- У каждого `SKILL.md` обязателен YAML-фронтматтер с `name` и `description`,
  иначе скилл молча выпадает из загрузки (видно в `skills list` как `Skipped`).
- `shadcn-vue` ставится из `unovue/shadcn-vue`; он читает `components.json`,
  который лежит в корне и описывает алиасы проекта.

## Definition of Done

- [ ] `cargo fmt` выполнен, диффа от автоформатирования нет
- [ ] `mise run check` — зелёные (biome + vue-tsc + clippy + `cargo test --workspace`)
- [ ] `mise run gen-types` — `src/shared/api/ipc.gen.d.ts` актуален (если менялись DTO)
- [ ] Новые DTO в `ipc_contract.rs` покрыты ts-rs и командой в `crates/app`
- [ ] Команда зарегистрирована в `generate_handler!` и в таблице `docs/ipc.md`
- [ ] UI-тексты добавлены в оба локаля (`src/shared/config/i18n/locales/{ru,en}.json`)
- [ ] `jj diff` содержит только то, что относится к задаче
- [ ] Коммит сделан **только если пользователь попросил**, гейт до этого зелёный
