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
| `mise run check-onboarding` | у каждого экрана есть тур, тексты обучения и «Что нового» есть в обоих локалях (правило 12) |
| `mise run check-ui` | общие компоненты вместо своей разметки, токены вместо сырых цветов, `<Money>` вместо `formatMoney` в шаблоне, `meta` с исходом у каждой мутации, лимит размера файлов (правило 15); `dvh` вместо `vh`, `*-safe` у `fixed` по краю, широкие ширины только с префиксом, раскрытие по наведению только с `pointer-fine:` (правило 16); слушатели без очистки, монтирование по наведению, подмена кнопок, запросы без `placeholderData`, `RouterView :key`, `backdrop-blur`, фиксированные треки сетки, шкалы `z-*`/размеров, `--container-*` в `@theme` (правило 17) |
| `mise run check-forms` | у форм кнопка сохранения не заблокирована проверкой полей, поля проверяются при сохранении (правило 13); счёт, актив и валюта выбираются готовыми компонентами (правило 14) |
| `mise run changelog` | обновить список коммитов в `## [Unreleased]` CHANGELOG.md (git-cliff) |
| `mise run bump` | следующая версия (`auto`/`patch`/`minor`/`major`/`X.Y.Z`) во все файлы + раздел CHANGELOG |
| `mise run check-platforms` | компиляция (clippy) под linux, windows, android, macos; `SKIP` — машина эту платформу проверить не может (правило «Сборка на всех платформах») |
| `mise run check-ci` | CI-скрипты: `*.ps1` только ASCII, shell-скрипты разбираются (`scripts/ci/lint-ci.sh`) |
| `mise run check-layout` | приложение на мок-бэкенде в headless Chromium на 1920…720 и 390/360, тёмная и светлая тема: нет переполнения и прокрутки вбок, наведение и загрузка ничего не сдвигают, диалог открывается на месте, слушатели не копятся, «Назад» закрывает слой, цели касания ≥ 40 px (правило 17); `--quick` — 1280 и 390, `--report` — без падения; скриншоты в `.cache/check-layout/` |
| `mise run check` | lint + typecheck + onboarding + forms + ui + layout (`--quick`) + ci + clippy + tests |
| `mise run build` | тесты + typecheck + `pnpm tauri build` (deb/rpm/AppImage) |
| `mise run build:all` | бандлы всех платформ (linux, pacman, flatpak, android, windows без MSI, macos на Mac) в CI-образах через podman, в `.cache/build-all/builds/` (`docs/ci.md`, «Локально») |
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
  src/vault_io/       # файл vault на любой ОС: local (atomic_write, lock, .bak), путь|URI, импорт, зеркало (docs/storage-platforms.md)
  src/pricing/        # PricingService: fetch_prices/fetch_fx (чистые, без мутации doc)
  src/service.rs      # UnlockedVault: мутации, холдинги, портфель, история, PnL
  src/market/         # OHLCV-свечи Binance для графиков (жило в sim/, лаборатория удалена)
  src/ipc_contract.rs # DTO для команд (входы/выходы IPC)
  src/bin/gen_types.rs# генератор src/shared/api/ipc.gen.d.ts (ts-rs export_all_to + слияние)
crates/plugin-saf/    # Android: выбор документа с долговременным правом (Kotlin), только target android
crates/app/           # Tauri shell: builder, commands/*, tauri.conf.json, capabilities/
  src/commands/       # auth, stores, transactions, categories, contacts, planning, analytics, pricing, portfolio, calc, settings, exchange
  src/platform_fs/    # ЕДИНСТВЕННОЕ место с файловыми диалогами, content:// и каталогом vault: desktop/mobile, копия во внешнем файле
  src/scheduler.rs    # фоновый поток: импорт Bybit, автоматизации, алерты, напоминания
  src/lib.rs          # tauri::Builder + generate_handler![…] (список ВСЕХ команд), AppState (vault: Mutex<Option<UnlockedVault>>, vault_path)
src/                  # фронтенд (Vue 3 + Pinia + TanStack Query + vue-i18n + ECharts), FSD v2.1
  main.ts             # bootstrap: createApp + роутер/пиния/vue-query/i18n
  app/                # верхний слой: точка входа и инфраструктура
    providers/        # router/, query/ — инфраструктурные провайдеры
    styles.css        # глобальные стили (Tailwind v4, base)
  pages/              # экраны, по одному на маршрут app/providers/router; ТОЛЬКО композиция
    dashboard/ stores/ wallet/ wallet-settings/ transactions/ planning/ analytics/ directory/ unlock/ app-shell/
  widgets/            # композитные блоки страниц (store-header, dashboard-total, sidebar-wallets, …)
  features/           # пользовательские действия (vault-unlock, store-create, price-refresh, …)
  entities/           # бизнес-сущности: store/, transaction/, asset/, currency/
  shared/             # нижний слой, ничего не импортирует выше себя
    ui/               # примитивы shadcn-vue/Reka: ПАПКА на компонент + index.ts
    lib/              # утилиты: format, echarts, theme, locale, transactions, vault-error…
    api/ipc.ts        # ЕДИНСТВЕННАЯ точка вызова Tauri (call/onEvent); api/mock/ — мок для pnpm dev, грузится лениво
    api/ipc.gen.d.ts  # AUTO-GENERATED, пересоздаётся mise run gen-types
    store/            # Pinia: vault.ts, settings.ts (глобальное состояние)
    config/i18n/locales/  # ru.json + en.json, плоские точечные ключи; набор ключей должен совпадать
.agents/skills/       # скиллы агентов, ВСЕ здесь (см. таблицу ниже)
mise.toml             # tools + env + tasks
.gitlab-ci.yml        # CI/CD: проверки, сборки Linux/Windows/Android, Flatpak/pacman, релиз (docs/ci.md)
scripts/ci/           # логика CI-job'ов (bootstrap, сбор бандлов, подпись Android, релиз, AUR)
scripts/local/        # build-all.sh: локальная сборка всех платформ тем же кодом, что CI
packaging/            # flatpak/ (манифест + metainfo), aur/ (PKGBUILD.in + render.sh), linux/ (postrm deb/rpm)
docs/                 # architecture, data-model, ipc, security, roadmap, ui-design, json-editor, ci
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
5. **Схема vault-файла: совместимость или миграция.** Существующие vault
   обязаны открываться и сохранять данные после обновления. Аддитивное поле
   добавляй с `#[serde(default)]`: JSON и MessagePack (release с v2) хранят
   имена полей, бамп не нужен. Не переименовывай, не удаляй и не меняй смысл
   сохранённого поля или варианта enum. Если правка несовместима — бампни
   `VAULT_FORMAT_VERSION` **вместе с миграцией**: заморозь текущую схему в
   `storage::vN`, добавь шаг и тесты на JSON и бинарный формат (так решил
   пользователь 2026-10-11; бамп без миграции недопустим). Старый bincode
   (`FMT_BIN`) только читается. Подробно — скилл `vault-crypto`.
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

12. **Обучение — часть фичи, а не доработка потом.** Видимая пользователю
    новая или изменённая функция (экран, диалог, действие, поле, смена
    поведения) выходит с туром (`tours.ts`, `data-tour`), подсказками на месте,
    пустым состоянием с первым шагом, пунктом «Что нового» и текстами в обоих
    локалях; старые шаги и подсказки обновляются. Внутренняя правка — строка
    `Onboarding: n/a — <причина>` в коммите. Проверка — `mise run
    check-onboarding`. Подробно — скилл `onboarding`.

13. **Формы: обязательные поля помечены, ошибки — под полем при сохранении.**
    Обязательное поле с красной `*`; кнопка сохранения **активна** (её
    блокирует только идущее сохранение или показанный рядом отказ бэкенда);
    нажатие с неверным полем показывает сообщение под ним и ставит фокус —
    `useFormValidation` + валидаторы `shared/lib/form`; что бэкенд отвергнет по
    известному правилу, форма проверяет сама. Проверка — `mise run
    check-forms`. Подробно — `ui-design-system/references/forms.md`.

14. **Счёт, актив и валюта выбираются только готовыми компонентами:**
    `StoreNodeSelect` / `AccountRefPicker`, `AssetInput` / `AssetSelect`,
    `CurrencyPicker` — в формах, фильтрах, настройках, полях нод. Не хватает
    возможности — пропс в компонент, а не список рядом. Проверка — `mise run
    check-forms`. Подробно — `ui-design-system/references/forms.md`.

15. **Интерфейс собирается из общих компонентов и отвечает на каждое
    действие.** Таблица, пустое состояние, загрузка/ошибка, диалог формы,
    шапка страницы, сумма и P&L — компоненты shadcn-vue и общие компоненты
    `shared/ui` (каталог — скилл `ui-design-system`), не своя разметка;
    повторяющийся блок выносится в общий компонент. Каждая мутация
    описывает исход в `meta` (`success`/`error`, для обратимого — `undo`;
    `silent` — с причиной), уведомления показывает `MutationCache`. Цвета —
    токены (`text-success`, `text-destructive`…), файл — одна смысловая
    часть (`.vue` до 250 строк, `.ts` до 300). Проверка — `mise run
    check-ui`; старые долги — `scripts/check-ui.baseline.json`, хуже
    становиться не могут.

16. **Интерфейс работает на телефоне.** Граница — `md` (768 px, во фронте
    `useIsPhone()`); отступы строки состояния, выреза и панели жестов —
    только `*-safe` / `var(--safe-*)`; высота — `dvh` или `h-full`, не `vh`;
    ни одного действия только с клавиатуры, наведения или правого клика
    (на касании — «⋯» `ActionMenu`); диалог на телефоне — шторка снизу;
    системный «Назад» закрывает верхний слой (`shared/lib/back-stack`):
    свой оверлей вне Reka регистрируется `pushBackLayer`. Цели касания
    (≥ 40 px) задаются в вариантах общих компонентов, а не в файлах экранов;
    скрытое до наведения — только с `pointer-fine:` (`pointer-fine:opacity-0`).
    Проверка — `mise run check-ui`, `mise run check-layout` и глазами на 390
    и 360 px с эмуляцией отступов. Подробно — `ui-design-system/references/mobile.md`.

17. **Интерфейс не двигается сам.** Наведение, фокус, нажатие, загрузка и
    обновление данных, появление полосы прокрутки не меняют размер и место
    соседей: место под кнопки строки зарезервировано, элемент управления не
    подменяется другим (`v-if`/`v-else`), спиннер не расширяет кнопку,
    скроллер со `scrollbar-gutter: stable`. Запрос с меняющимся ключом
    держит прежние данные (`placeholderData: keepPreviousData`), скелетон —
    высотой с контент. Подписка на событие бэкенда — только
    `useBackendEvent`, таймер очищается в `onScopeDispose`. Анимация
    оверлея не трогает `translate`/`transform` позиционированного элемента;
    `backdrop-filter` запрещён (WebKitGTK замирает на нём). Проверка — `mise
    run check-ui` (статически) и `mise run check-layout` (приложение в
    headless Chromium: переполнение, сдвиг при наведении и загрузке, утечки
    слушателей, «Назад», цели касания). Подробно —
    `ui-design-system/references/stable-layout.md`.

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

## CI/CD (GitLab)

Пайплайн — `.gitlab-ci.yml`, вся логика job'ов — в `scripts/ci/*`, манифесты
упаковки — в `packaging/`; устройство, переменные — `docs/ci.md`; ветки и
выпуск — `docs/workflow.md`. Перед правкой CI загружай скиллы
`gitlab-ci-generator` и `gitlab-ci-validator`, после — прогоняй валидатор;
пайплайны и логи смотри через `glab` (скилл `glab`).

- **Ветки.** `feature/*` (от `development`) и `hotfix/*` (от `main`) — только
  проверки; `development` — пре-релиз `X.Y.Z-N` на каждый пуш; `main` —
  релиз `X.Y.Z` на каждый мерж. Работа идёт через MR, теги ставит CI.
- **Сборки ждут проверки.** Каждый `build:*` в `needs` держит все проверки и
  `version`: красная проверка не должна доходить до сборки и выпуска.
- **Версия и CHANGELOG — только через `mise run bump` / `mise run changelog`**
  (git-cliff по Conventional Commits): версию в файлах руками не меняют, тип
  коммита (`feat`/`fix`/`perf`/…) решает, куда он попадёт и какой будет
  следующая версия. Релиз из `main` без раздела `## [X.Y.Z]` в
  `CHANGELOG.md` CI не выпустит.
- **`*.ps1` — только ASCII** (Windows PowerShell 5.1 читает их как cp1252:
  «—» ломает разбор); `mise run check-ci` / job `ci:lint` это ловят.

- **CI повторяет локальный гейт.** Новая проверка в `mise run check`
  добавляется и в job `frontend:check` или `rust:check`; CI не должен быть
  мягче `mise run check`. Логику держи в скрипте `scripts/ci/*`, а не в
  многострочном `script:` — так её можно запустить и проверить локально.
- **Версии инструментов — только из `mise.toml`.** CI ставит node/pnpm/rust
  через mise (Windows-скрипт читает те же строки); не хардкодь версии в YAML.
- **CI-образы.** Linux-job'ы идут в готовых образах
  (`scripts/ci/ci.Dockerfile`, теги `ci-node`/`ci-linux`/`ci-android`).
  Поменял Dockerfile, `scripts/ci/apt-packages.sh` или `[tools]` в
  `mise.toml` — обнови `CI_IMAGE_TAG` в `.gitlab-ci.yml` на вывод
  `scripts/ci/image-tag.sh`, иначе `ci:image` упадёт. Новый apt-пакет —
  только в `apt-packages.sh` (его читают и образ, и `setup-linux.sh`).
- **Скорость не терять.** Не убирай `opt-level = 3` у `argon2`/`blake2` в
  `[profile.dev.package.*]` (без него тесты в 13 раз дольше); кэши
  (sccache, cargo-registry, pnpm) пишут только `development` и `main`; новый job — с
  `rules: changes` по своим путям и `interruptible`, если его можно бросить.
  Подробно — `docs/ci.md`, раздел «Скорость».
- **Платформенный код.** Трей, автозапуск, глобальные клавиши, геометрия окна
  и OS keyring есть только на десктопе: код — под `#[cfg(desktop)]` с
  заглушкой под `#[cfg(mobile)]`, плагины — в
  `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`
  `crates/app/Cargo.toml`. Иначе ломается `build:android`. Проверка без CI —
  `mise run check-platforms` (см. «Сборка на всех платформах»).
- **Идентификаторы не меняются.** Два канала — два приложения рядом
  (`scripts/ci/channel.sh`, `docs/ci.md` «Каналы»): релиз из `main` —
  `mwt.desktop` (десктоп; от него зависит путь к vault), `mwt.app` (Android),
  `mwt.app.MultiWalletTracker` (Flatpak); dev из `development` и локальные —
  `dev.mwt.desktop`, `dev.mwt.app` (`tauri.conf.json`,
  `tauri.android.conf.json`), `dev.mwt.MultiWalletTracker`. Идентификатор в
  Rust — только `app_id::get()`, не константа. Смена = потеря данных у
  пользователей или невозможность обновиться. Прежнее имя TradeApp (`dev.tradeapp.desktop`)
  переносится один раз при запуске — `crates/app/src/legacy.rs`; не удаляй
  его, пока у кого-то может остаться старая установка.
- **Имя.** Приложение — Multi Wallet Tracker (MWT): `productName`, бинарник и пакеты
  `multi-wallet-tracker`, AUR `multi-wallet-tracker-bin`; dev-канал —
  «Multi Wallet Tracker Dev», `multi-wallet-tracker-dev`. Rust-крейты `trade-core`/`trade-app`
  остаются внутренними именами. Магические байты vault и формат экспорта
  `trade-app-workflow` не трогать — от них зависит чтение старых файлов.
- **Выпуск — тег `vX.Y.Z`.** Версия одинакова в `crates/app/tauri.conf.json`,
  `[workspace.package]` `Cargo.toml` и `<release>` в
  `packaging/flatpak/*.metainfo.xml`; `version:check` сверяет тег. Тег и
  `jj git push` — только по просьбе пользователя.
- **Секреты — только в CI/CD-переменных** (ключ подписи Android, SSH-ключ
  AUR, `RELEASE_TOKEN`), masked. Ни ключей, ни keystore в репозитории.
- **Генерируемое не коммитится:** Gradle-проект `crates/app/gen/android/`,
  `release/`, `.cache/`, `.flatpak-*` (в `.gitignore`).

## Проверки и коммиты (Jujutsu)

Репозиторий под **jj** (colocated с git). Рабочая копия автоматически
снапшится в change `@` — `git add` не существует и не нужен.

### Рабочие копии: только рядом, в `../work-app-<задача>`

Пользователь должен видеть, где идёт работа, и иметь возможность открыть её
в своём редакторе. Поэтому:

- Небольшая задача — прямо в основной копии: `jjt <slug>` → правки →
  гейт → `jjc "<тема>"` (скилл `jj-tooling` из `../jj-tools`).
- **Проверки запускает агент сам, а не jj-tools.** Скрипты `jjc`,
  `jjsplit`, `jjw-finish` гейт не запускают: агент прогоняет `cargo fmt` +
  `mise run check` (и `check-platforms`) своими командами, чтобы
  пользователь видел каждый прогон и его итог.
- Крупная задача (много файлов, эксперименты, несколько коммитов) или
  параллельная работа — **отдельный jj workspace рядом с репозиторием**:
  `jjw-start <slug>` создаёт `../work-app-<slug>/` и закладку `<slug>`.
  Путь к этой папке агент называет пользователю в первом же сообщении
  о задаче.
- **Запрещено** создавать рабочие копии внутри репозитория или в скрытых
  каталогах: `git worktree add`, `.claude/worktrees/*`, `EnterWorktree` и
  `isolation: worktree` в Claude Code. Такую копию пользователь не может
  открыть и переключиться в неё, не видно, что в ней меняется, а её
  `biome.json` ломает `mise run check` в основной копии.
- Закончил — гейт и `mise run check-platforms` в этой папке, затем
  `jjw-finish <slug>` **из основной копии** (запущенный изнутри рабочей
  копии он ничего не переносит): переносит change'ы в основную копию и
  удаляет папку. После него — снова гейт в основной копии, на влитом
  дереве. Копия не переживает свою задачу; `jjws` проверяет, что хвостов
  не осталось.
- Коммиты внутри `../work-app-*` — обычными `jj describe` + `jj new`
  (`jjc` там отказывается работать), гейт — `mise run check` в этой папке.

### Гейт перед коммитом

```bash
cargo fmt             # сначала форматирование, потом проверки
mise run check        # biome check + vue-tsc --noEmit + clippy + cargo test --workspace
```

Гейт зелёный → коммитим. Не зелёный → коммит запрещён, сначала чини.
Никогда не «фиксим» падение теста удалением или ослаблением проверки.

### Сборка на всех платформах

После **последней** правки, перед влитием в основную копию (`jjw-finish`) и
перед коммитом, проверь, что проект компилируется на всех платформах:

```bash
mise run check-platforms   # linux, windows (cargo-xwin), android (NDK или образ ci-android), macos
```

- Код под `#[cfg(...)]`, платформенные зависимости и `platform_fs` на хосте
  не компилируются: `mise run check` их не видит, ошибка всплывёт только в
  CI или у пользователя (так `unused import` под Android прошёл в
  `fix(sync)` 2026-10-18).
- Любая правка после проверки — проверь заново: считается только прогон на
  финальном состоянии.
- `FAIL` — влитие и коммит запрещены, как при красном гейте.
- `SKIP` допустим только как ограничение системы (macOS на Linux без
  `MACOS_BUILD_HOST`, нет cargo-xwin / NDK / podman). Не «чини» его
  пропуском: назови пользователю пропущенную платформу и причину.
- Полные бандлы — `mise run build:all`; для проверки компиляции он не нужен.

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
| `feature-sliced-design` | размещение кода по слоям и границы импортов (методология; отличия проекта — в `frontend-feature`) |
| `ipc-command` | новая или изменённая Tauri-команда (DTO → core → хендлер → gen-types) |
| `vault-crypto` | формат vault, миграции, крипта, `Amount`, секреты |
| `frontend-feature` | архитектура фронтенда: слои, Query/Pinia, IPC и мок, ключи запросов, i18n, маршруты, графики |
| `ui-design-system` | вид и поведение UI: каталог общих компонентов, токены, формы, уведомления о действиях, загрузка/ошибка/пусто, таблицы и деньги, клавиатура, тексты |
| `shadcn-vue` | любой shadcn-компонент: добавить, найти, починить, скомпоновать |
| `json-editor` | редактор vault в формате JSON (Настройки → Просмотр JSON) |
| `pricing-offline` | цены/FX, провайдеры, кэш, офлайн-деградация |
| `onboarding` | любая видимая пользователю фича: туры, подсказки, пустые состояния, «Что нового», центр обучения (правило 12) |
| `gitlab-ci-generator` | новый или изменённый job, правила, кэш в `.gitlab-ci.yml` |
| `gitlab-ci-validator` | проверка `.gitlab-ci.yml` перед коммитом (синтаксис, best practices, безопасность) |
| `glab` | пайплайны, логи job'ов, MR и релизы в GitLab через CLI `glab` |

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
- `.claude/skills/` — ссылки на `.agents/skills/<name>`, по одной на скилл,
  чтобы Claude Code находил скиллы сам (инструмент Skill), а не только по
  таблице выше. Собирает их `scripts/link-claude-skills.sh`: после
  `skills add` удали копию `agent/`, которую создаёт CLI, и перезапусти
  скрипт. Ссылки — на все скиллы без исключений; сами файлы скиллов в
  `.claude/` не кладутся.
- План и список задач — **не в репозитории**. Скиллы из реестров
  (`planning-and-task-breakdown`, `spec-driven-development`) по умолчанию
  пишут `tasks/plan.md` и `tasks/todo.md` — здесь это запрещено: план — файл
  в `~/.claude/plans/` (в OpenCode и Codex — их каталог планов вне
  репозитория), подзадачи — список задач сессии. Каталог `tasks/` в рабочей
  копии не создавай; появился — удали до коммита.

## Definition of Done

- [ ] `cargo fmt` выполнен, диффа от автоформатирования нет
- [ ] `mise run check` — зелёные (biome + vue-tsc + clippy + `cargo test --workspace`)
- [ ] `mise run check-platforms` на финальном состоянии — без `FAIL`; каждый
      `SKIP` назван пользователю с причиной
- [ ] `mise run gen-types` — `src/shared/api/ipc.gen.d.ts` актуален (если менялись DTO)
- [ ] Новые DTO в `ipc_contract.rs` покрыты ts-rs и командой в `crates/app`
- [ ] Команда зарегистрирована в `generate_handler!` и в таблице `docs/ipc.md`
- [ ] UI-тексты добавлены в оба локаля (`src/shared/config/i18n/locales/{ru,en}.json`)
- [ ] Обучение к фиче (правило 12, скилл `onboarding`): тур, подсказки, пустые
      состояния, «Что нового» обновлены и `mise run check-onboarding` зелёный —
      или в коммите строка `Onboarding: n/a — <причина>`
- [ ] Формы (правило 13): обязательные поля с `*`, кнопка сохранения активна,
      ошибки под полями через `useFormValidation`; `mise run check-forms` зелёный
- [ ] Выбор счёта, актива, валюты (правило 14) — `StoreNodeSelect` /
      `AccountRefPicker` / `AssetInput` / `AssetSelect` / `CurrencyPicker`
- [ ] Общие компоненты и исход каждого действия (правило 15):
      `mise run check-ui` зелёный, база долгов не выросла
- [ ] Стабильность (правило 17): `mise run check-layout` зелёный (полный, не
      только `--quick` из `check`), если менялся интерфейс; в окне Tauri на
      Linux (WebKitGTK) проверены наведение на строки, смена фильтра,
      открытие диалогов, быстрые сохранения — ничего не прыгает и не замирает
- [ ] Телефон (правило 16): экран проверен на 390 и 360 px с эмуляцией
      отступов, без горизонтальной прокрутки страницы, каждое действие
      доступно касанием
- [ ] `jj diff` содержит только то, что относится к задаче
- [ ] Коммит сделан **только если пользователь попросил**, гейт до этого зелёный
