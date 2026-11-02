# CI/CD (GitHub Actions) и платформы

Как работать с ветками `feature/*`, `hotfix/*`, `development`, `main` и
выпуском — [workflow.md](workflow.md). Здесь — устройство CI.

CI и выпуск живут в GitHub Actions (`.github/workflows/`), логика — в
`cargo xtask` (`crates/xtask`), скриптах `scripts/ci/*` и `.github/scripts/*`,
манифесты упаковки — в `packaging/`. Версии Rust/Node/pnpm берутся из
`mise.toml` (на Linux через сам mise, на Windows скрипт читает их из файла).
GitLab (`Reckeck/trade-app`) — только зеркало без CI.

## Workflows

| Файл | Когда идёт | Что делает |
|---|---|---|
| `ci.yml` | PR в `development`/`main`, вручную, `workflow_call` из `release.yml` | проверки: Rust, фронтенд, layout/links шардами, Android, clippy под Windows/macOS, итог `ci-ok` |
| `images.yml` | вызывается из `ci.yml`, вручную | собирает и пушит CI-образы в ghcr, если такого тега ещё нет |
| `release.yml` | пуш в `development` (пре-релиз `X.Y.Z-N`) и `main` (релиз `X.Y.Z`), вручную | версия, проверки, сборки, пакеты, публикация GitHub Release |
| `audit.yml` | PR, пуш в `development`/`main`, ежедневно, вручную | аудит зависимостей (`cargo xtask audit`) |
| `dependabot-merge.yml` | воскресенье, вручную | влитие зелёных обновлений Dependabot |
| `cache-cleanup.yml` | закрытие PR, удаление ветки | удаляет кэши ветки |

Фиче-ветки и хотфиксы получают только проверки (через PR); сборки и выпуск —
только `development` и `main` и только после зелёных проверок. Теги
`vX.Y.Z` создаёт job `publish` вместе с релизом. Проверка workflow —
`mise x actionlint@1.7.12 -- actionlint`.

## Скорость

Что делает CI быстрым и что не надо ломать:

- **Готовые образы.** Job'ы идут в образах из `scripts/ci/ci.Dockerfile`
  (ghcr.io, `ghcr.io/<владелец>/<репозиторий>/ci-<вид>:<тег>`): `ci-node`
  (mise + node/pnpm, фронтенд), `ci-linux` (+ библиотеки Tauri, Rust,
  sccache), `ci-android` (+ JDK 17, SDK/NDK, Rust-таргеты). Job не ставит ни
  apt-пакеты, ни тулчейн — `setup-linux.sh` только подключает кэши. Тег
  образа — хеш входов (Dockerfile, `apt-packages.sh`, `install-chrome.sh`,
  `[tools]` из `mise.toml`), его печатают `scripts/ci/image-tag.sh` и
  `mise run ci-image-tag`. Тег вручную нигде не прописывается: `images.yml`
  (`.github/scripts/build-images.sh`) сам собирает недостающие образы, уже
  запушенный тег не пересобирается (у ручного запуска есть `force`).
- **sccache.** Скомпилированные крейты лежат в кэше Actions (`.cache/sccache`,
  по полосам: `ci.yml`, job `deps`, ключ — `.github/scripts/lane-key.sh`,
  включает тег образа) и возвращаются через `RUSTC_WRAPPER=sccache`. Кэш
  полосы пишет только `deps` (и только когда для ключа его ещё нет),
  остальные jobs читают. Кэш виден своей ссылке, базовой ветке PR и
  `development`; `main` идёт без кэшей (релиз собирается с нуля).
- **sccache локально.** `cargo xtask` (`mise run check`, `check-platforms`)
  ставит `RUSTC_WRAPPER=sccache` всем cargo-задачам на хосте и запускает
  сервер до графа; кэш один на машину: `~/.cache/sccache/host` (16 GB,
  все рабочие копии). Контейнеры Android и `build:all` получают
  `~/.cache/sccache/containers` (4 GB) как `/sccache`; каталоги раздельные,
  потому что ключи не пересекаются, а два сервера в одном каталоге вычищали
  бы записи друг друга. Заданные `RUSTC_WRAPPER`/`SCCACHE_DIR` не
  перетираются; без sccache в PATH сборка идёт как раньше и пишет подсказку
  (`mise use -g sccache`); `XTASK_SCCACHE=0` выключает его везде. В конце
  прогона xtask печатает `sccache: N requests, H hits, M misses`.
  Инкрементальные крейты воркспейса sccache не кэширует, только зависимости.
  В CI ничего не меняется: `setup-linux.sh` берёт `SCCACHE_DIR` из
  окружения, а по умолчанию остаётся `.cache/sccache`.
- **Тесты быстрые.** KDF vault (Argon2id, 64 MiB x 3) идёт на каждом
  сохранении; `argon2`/`blake2` собираются с `opt-level = 3` даже в debug
  (`Cargo.toml`), иначе `cargo test` шёл ~5,5 мин вместо ~30 с. Параметры
  KDF не менялись.
- **Новый пуш отменяет старый прогон** (`concurrency` с `cancel-in-progress`
  во всех ветках, кроме `main`): выпуск из `main` не прерывается.
- **Пре-релизы — без LTO.** Канал dev (пре-релизы из `development`, ручные и
  локальные сборки) собирается с `CARGO_PROFILE_RELEASE_LTO=off` и
  `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`: финальная оптимизация идёт
  параллельно, а не в один поток. Переменные выставляет сам `cargo xtask
  build` по каналу (`Channel::build_env`; в контейнер они пробрасываются) и
  `scripts/ci/build-windows.ps1` по `RELEASE_CHANNEL`; значение, заданное в
  окружении явно, главнее. Релизы из `main` (prod) собираются с полным LTO и
  одной единицей (`[profile.release]` в `Cargo.toml`) — самый маленький и
  быстрый бинарник.
- Без debug-info в проверках (`CARGO_PROFILE_DEV_DEBUG=0`).

## Выпуск версии

Пре-релизы — каждый пуш в `development`, релиз — мерж в `main`; шаги — в
[workflow.md](workflow.md#выпуск). Версия сборки:
`cargo xtask version --channel dev|prod|none` (dev — `X.Y.Z-N`, где `N` — последний опубликованный тег того же `X.Y.Z` плюс один; ручные — `X.Y.Z-<номер прогона>`,
prod — `X.Y.Z`); в сборку она попадает через `tauri build --config`
(`cargo xtask tauri-config`), вместе с Android `versionCode` = номер
запуска (`GITHUB_RUN_NUMBER`).

### Выпуск через xtask (GitHub)

Служебные шаги выпуска вынесены в команды xtask (`crates/xtask/src/publish/`),
каждая понимает `--dry-run` (печатает команды, ничего не делает на GitHub):

| Команда | Что делает |
|---|---|
| `cargo xtask publish --version V --dir D [--pre]` | `SHA256SUMS`, затем `gh release create`; `--pre` для пре-релиза `X.Y.Z-N` из `development` |
| `cargo xtask prune-prereleases [--keep N]` | оставить `N` последних пре-релизов (по умолчанию `PRERELEASES_KEPT` или 10), остальные и их теги удалить |
| `cargo xtask backmerge --version V` | PR `main` в `development` после выпуска |
| `cargo xtask publish-aur --version V --dir D` | пуш `multi-wallet-tracker-bin` в AUR (`AUR_SSH_PRIVATE_KEY`) |

Токен — `GH_TOKEN` (или `GITHUB_TOKEN`).
Вызывает их `.github/workflows/release.yml` (раздел «Проверки в ci.yml» и
«release.yml» ниже).

### Каналы: релиз и dev-приложение

Сборки `main` (`RELEASE_CHANNEL=prod`) и `development` (`dev`) — два разных
приложения: ставятся рядом, данные у каждого свои. Имена задаёт
`crates/xtask/src/channel.rs`, в сборку их кладёт оверлей `cargo xtask tauri-config`
(`build-windows.ps1` повторяет их, `mise run check-ci` сверяет):

| | prod (`main`, релиз) | dev (`development`, ручные, локальные) |
|---|---|---|
| Имя (`productName`) | Multi Wallet Tracker | Multi Wallet Tracker Dev |
| Бинарник, пакеты | `multi-wallet-tracker`, AUR `multi-wallet-tracker-bin` | `multi-wallet-tracker-dev`, pacman `multi-wallet-tracker-dev-bin` |
| Десктоп (путь к vault) | `mwt.desktop` | `dev.mwt.desktop` |
| Android | `mwt.app` | `dev.mwt.app` |
| Flatpak | `mwt.app.MultiWalletTracker` | `dev.mwt.MultiWalletTracker` |

- `tauri.conf.json` / `tauri.android.conf.json` держат значения dev: так
  собираются `mise run dev`, `mise run build` и сборки без канала (`none`).
- Ключ keyring и сокет быстрого добавления привязаны к идентификатору сборки
  (`crates/app/src/app_id.rs`), автозапуск — к `productName`, поэтому
  удаление одного приложения не трогает другое (`packaging/linux/postrm.sh`,
  `crates/app/windows/autostart-cleanup.wxs`).
- Перенос данных TradeApp (`legacy.rs`) делает только dev: релиз под
  `mwt.desktop` начинает с чистого каталога.
- Манифест Flatpak в репозитории — dev; релизный `build-flatpak.sh`
  рендерит из него под своим ID.

### Оверлей и имена канала для скриптов сборки

Скрипты `scripts/ci/build-*.sh` не вызывают xtask (в контейнерах pacman и
flatpak нет cargo), поэтому имена канала и конфиг Tauri приходят к ним
снаружи:

- `MWT_TAURI_CONFIG` — путь к оверлею (`tauri build --config`), его пишет
  `cargo xtask tauri-config` (по умолчанию `.cache/tauri-ci.json`; `cargo xtask
  build` пишет свой файл на платформу, `.cache/tauri-ci-<платформа>.json`);
- `MWT_APP_NAME`, `MWT_APP_BIN`, `MWT_APP_ID`, `MWT_APP_ANDROID_ID`,
  `MWT_APP_FLATPAK_ID`, `MWT_APP_ICONS` — значения канала из `Channel::env()`
  (`crates/xtask/src/channel.rs`).

## Проверки в ci.yml

Проверки идут в `.github/workflows/ci.yml` (образы — `images.yml`,
очистка кэша — `cache-cleanup.yml`), выпуск — в `release.yml`.
`ci.yml` запускается на PR и вызывается из `release.yml` (`workflow_call`):
пуш в `development`/`main` проверяет именно этот вызов, отдельного запуска
`ci.yml` на пуш нет.

- `check-linux` (`cargo xtask check --no-frontend --no-platforms`) и
  `check-frontend` (`cargo xtask check --frontend`) — два разных job'а в
  образе `ci-linux`: Rust-проверки и веб-часть (lint, typecheck, vitest,
  статические проверки, e2e) идут параллельно.
- `frontend-layout` — полные `check-layout` и `check-links` (все ширины, обе
  темы) матрицей из 6 шардов: `cargo xtask check layout links --full-browser
  --shard K/6 --jobs 1`. Работа поделена на мелкие юниты (пачка ~5 видов темы
  x ширины, пачка ~10 оверлеев, страница links); K-й шард берёт K-ю долю по
  весам из `tests/e2e/layout/weights.json` и `tests/e2e/links/weights.json`
  (секунды юнита, жадное деление «тяжёлый на самый лёгкий», детерминированно).
  Юнита нет в файле — вес по умолчанию (медиана), лишний ключ игнорируется.
  Веса обновляет полный локальный прогон: `mise run check-layout --
  --update-weights` (и `check-links`), результат коммитится; замеры каждого
  прогона — `.cache/check-<имя>/timings.json`. Локально то же деление между
  процессами одной машины — `--jobs N` (один Vite, свой Chromium на процесс);
  шард и `--jobs` складываются. `fail-fast: false`: упавший шард не гасит
  остальные; логи и снимки — артефакт `frontend-layout-<K>-logs`.
- macOS в CI на паузе: job `clippy-macos` оставлен в файле, но в матрице
  закомментирован.
- `SCCACHE_BASEDIRS` (`.github/scripts/ci-env.sh`) действует только на C/C++;
  на rustc sccache его не применяет.

### Влитие обновлений Dependabot (`dependabot-merge.yml`)

Dependabot открывает PR в `development` по понедельникам (`.github/dependabot.yml`):
minor и patch одной экосистемы — одним групповым PR, каждый major — отдельным. Каждое
воскресенье в 06:00 МСК (`cron: 0 3 * * 0`) `dependabot-merge.yml` вливает (squash) те
из них, у которых все проверки зелёные (и `ci-ok`), нет конфликта и обновление не major
(для `0.x` major — смена второй цифры; групповой PR без «from A to B» — только группы
`cargo-minor`, `npm-minor`, `actions`). Major, красные, идущие и конфликтующие PR
остаются на ручной разбор; конфликт после соседнего влития Dependabot сам перебазирует,
и PR вливается через неделю. Влитие токеном `GITHUB_TOKEN` не запускает push-workflow,
поэтому после влития скрипт сам запускает `release.yml` (dev, publish) на `development`.
Список влитых и пропущенных с причиной — в сводке прогона. Вручную: Actions →
dependabot-merge → Run workflow, `dry_run` только показывает, что было бы влито.
Логика — `.github/scripts/dependabot-merge.sh`.

### Аудит зависимостей (`audit.yml`)

`mise run audit` (`cargo xtask audit`) проверяет `Cargo.lock` (`cargo audit`,
нужен `cargo install cargo-audit --locked --version 0.22.2`) и `pnpm-lock.yaml`
(`pnpm audit --audit-level high`). Падают уязвимости, `unsound` и `yanked` у Rust и
`high`/`critical` у npm; `unmaintained` и мелкие npm-находки только печатаются.
В `mise run check` аудит не входит. Отдельный workflow `audit.yml` идёт на PR в
`development`/`main`, пуш в них, ежедневно (cron) и вручную; в `ci-ok` он не
входит: новая находка в нетронутой зависимости не должна блокировать чужие PR.

Исключение — запись `[[ignore]]` в `audit-ignore.toml` в корне: `id` (RUSTSEC
или GHSA; совпадает и с алиасом), `tool` (`cargo` или `pnpm`) и непустая
`reason` (почему принято и когда можно убрать). Запись, которая больше ничего не
подавляет, аудит печатает предупреждением: её надо удалить.

### release.yml

Пуш в `development` — пре-релиз `X.Y.Z-N` dev-канала, пуш в `main` — релиз
`X.Y.Z` prod-канала; PR выпуск не запускает. Граф (`needs`):

```
version, images ──► frontend ──► build (linux | android | windows) ──► package (pacman | flatpak)
   │      │                    │                                                     │
   │      │                    └──► build-macos (пауза)                              │
   │      └──► checks (ci.yml), параллельно со сборками ──► cancel-on-red (если красные)
   └──────────────────────────────────────────────────────────► publish ◄───────────────┘
                                                                  ├──► aur       (только prod)
                                                                  └──► backmerge (только prod)
```

- `version` — решает канал и нужен ли выпуск (`.github/scripts/release-plan.sh`),
  собирает `xtask` (единственный Rust-компилятор в запуске, бинарник идёт
  артефактом во все следующие job'ы), считает `X.Y.Z[-N]`
  (`cargo xtask version`: падает, если тег `vX.Y.Z` уже есть) и пишет оверлей
  Tauri для Windows. Android `versionCode` — `GITHUB_RUN_NUMBER` (только растёт); номер пре-релиза `N` dev — по тегам удалённого репозитория, а не номер прогона.
- `images` — образы CI (`images.yml`); от них зависят `frontend`, сборки и
  `checks`. Сборки проверок не ждут: идут параллельно с ними.
- `checks` — `ci.yml` целиком. `publish` требует `checks` в статусе
  `success`; красные проверки запускают `cancel-on-red` (`actions: write`,
  `gh run cancel`), и оставшиеся сборки гасятся.
- `frontend` — `pnpm build` один раз, `dist/` уходит артефактом; сборки зовут
  `--prebuilt-frontend` (оверлей без `beforeBuildCommand`).
- `build` — одна группа с `fail-fast: true`: первая упавшая сборка отменяет
  остальные (перезапуск — «Re-run failed jobs»). linux и android —
  `xtask build <платформа>` на раннере с образами `ci-linux`/`ci-android` из
  ghcr (docker), windows — `scripts/ci/build-windows.ps1` на `windows-latest`
  (MSI собирает WiX). Пре-релизы идут без LTO на 16 codegen units, релиз — с
  LTO из `[profile.release]`. Кэш sccache контейнерных сборок перед
  сохранением возвращается пользователю раннера (`chown`): docker пишет его
  от root. Ключ кэша (`lane-key.sh`) включает канал; на Windows
  `SCCACHE_IDLE_TIMEOUT=0` держит сервер живым на хвосте LTO.
- `package` — pacman и Flatpak из Linux-тарболла (`xtask build pacman|flatpak`,
  контейнер archlinux, Flatpak `--privileged`).
- `publish` — единственный job с `contents: write`: проверка набора файлов
  (`.github/scripts/check-release-files.sh`), `SHA256SUMS`, тег и релиз
  (`cargo xtask publish [--pre]`), затем `prune-prereleases` (оставить 10;
  ошибка не валит запуск).
- `aur` (prod) — `publish-aur` в контейнере archlinux; без секрета
  `AUR_SSH_PRIVATE_KEY` шаг пропускается с предупреждением.
- `backmerge` (prod, `pull-requests: write`) — PR `main` → `development`; токен
  `BACKMERGE_TOKEN` (если есть) иначе `GITHUB_TOKEN` (нужна настройка «Allow
  GitHub Actions to create and approve pull requests»); ошибка не валит запуск.
- `build-macos` написан, но выключен (`if: false`, как `clippy-macos`): включить
  — убрать `if: false`, добавить его в `needs` и в условие `publish`, третьим
  аргументом `check-release-files.sh` передать `macos`.

Файлы выпуска (`<v>` — версия, решение 12 плана):

| Платформа | Файлы |
|---|---|
| Linux | `multi-wallet-tracker_<v>_amd64.deb`, `multi-wallet-tracker-<v>-1.x86_64.rpm`, `multi-wallet-tracker-<v>-x86_64.AppImage`, `multi-wallet-tracker-<v>-linux-x86_64.tar.gz` (источник AUR) |
| Arch / Flatpak | `*.pkg.tar.zst` (pacman), `multi-wallet-tracker-<v>-x86_64.flatpak`; в AUR — `multi-wallet-tracker-bin` (prod) |
| Android | `multi-wallet-tracker-<v>-android-universal.apk` и `.aab` |
| Windows | `multi-wallet-tracker-<v>-x64-setup.exe` (NSIS), `multi-wallet-tracker-<v>-x64-<язык>.msi`, `multi-wallet-tracker-<v>-windows-x64-portable.zip` |
| macOS (пауза) | `multi-wallet-tracker-<v>-macos-universal.dmg`, `.app.tar.gz` |
| Все | `SHA256SUMS` |

**Секреты** (Settings → Secrets and variables → Actions):

| Секрет | Зачем | Без него |
|---|---|---|
| `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` | подпись APK/AAB; прежний keystore, иначе установленное приложение не обновится | при выпуске сборка Android падает; без выпуска APK подписан тестовым ключом (`*-testkey`), такие файлы не публикуются |
| `ANDROID_KEY_PASSWORD` | пароль ключа, если отличается от пароля хранилища | берётся пароль хранилища |
| `AUR_SSH_PRIVATE_KEY` | пуш в AUR | шаг пропускается с предупреждением |
| `BACKMERGE_TOKEN` | PAT/токен приложения для PR `main` → `development` (запускает проверки) | PR от `GITHUB_TOKEN` |
| `APPLE_*` | подпись macOS (когда включат) | ad hoc подпись |
| `TAURI_SIGNING_PRIVATE_KEY` | подпись обновлений: `latest.json` + `.sig` (решение 16) | не используется: плагина обновлений в приложении ещё нет |

Keystore не пишется на диск раннера: `ANDROID_KEYSTORE_BASE64` попадает в
контейнер переменной окружения (GitHub маскирует значение в логе),
`sign-android.sh` раскладывает его во временный файл внутри контейнера, и тот
исчезает вместе с контейнером.

**Запуск вручную** (Actions → release → Run workflow, или
`gh workflow run release.yml --ref <ветка> -f channel=dev -f publish=false`):
выбрать ветку, `channel` (`dev` или `prod`) и `publish`. С `publish: false`
(по умолчанию) идут `version`, проверки и все сборки, артефакты лежат в
запуске (`bundle-*`, 7 дней), `publish`, `aur`, `backmerge` пропускаются, тег и
релиз не создаются. Выпуск вручную разрешён только как `dev` на `development`
или `prod` на `main`. `dev` без выпуска получает версию `X.Y.Z-N` (тот же счёт по тегам), `prod` — как
релиз (падает, если `vX.Y.Z` уже выпущен или в CHANGELOG нет раздела).

Кэш: проверки внутри `checks` читают и пишут свои полосы, как в PR; у сборок
своих полос `deps` пока нет, поэтому release-сборки идут без кэша
скомпилированных крейтов (на `main` так и задумано, решение 26).

## Раннеры

- Проверки и Linux-сборки — `ubuntu-latest` с docker; сборки идут в образах
  `ci-linux`/`ci-android` из ghcr. Flatpak собирается в контейнере с
  `--privileged` (bubblewrap).
- Windows — `windows-latest` (`scripts/ci/build-windows.ps1`; MSVC Build Tools
  ставятся через choco, если их нет; MSI собирает WiX).
- macOS в CI на паузе (`clippy-macos`, `build-macos` выключены).
- Android-сборка длинная (release + LTO, три архитектуры —
  `ANDROID_TARGETS`): timeout 2 ч.
- `images.yml` нужен docker и `packages: write` у `GITHUB_TOKEN`.

## Секреты (Settings → Secrets and variables → Actions)

Секреты, которые читают workflows (кроме автоматического `GITHUB_TOKEN`):

| Секрет | Нужен для | Примечание |
|---|---|---|
| `ANDROID_KEYSTORE_BASE64` | релизная подпись APK/AAB | `base64 -w0 release.keystore` |
| `ANDROID_KEYSTORE_PASSWORD` | то же | |
| `ANDROID_KEY_ALIAS` | то же | |
| `ANDROID_KEY_PASSWORD` | то же | по умолчанию = пароль хранилища |
| `AUR_SSH_PRIVATE_KEY` | `aur` (`publish-aur`) | ключ, добавленный в аккаунт AUR |
| `BACKMERGE_TOKEN` | `backmerge` | PAT/токен приложения; иначе `GITHUB_TOKEN` |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | подпись macOS (job выключен) | base64 `.p12` Developer ID; без них — ad hoc |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | нотаризация macOS (job выключен) | пароль приложения Apple ID |

Подробности «без секрета» — таблица секретов в разделе «release.yml».

Без ключа Android APK подписывается одноразовым тестовым ключом и получает
суффикс `-testkey`: ставится для проверки, но не обновляет предыдущую
сборку и не годится для публикации. Ключ создаётся один раз и хранится вне
репозитория:

```bash
keytool -genkeypair -v -keystore release.keystore -alias mwt \
  -keyalg RSA -keysize 4096 -validity 10000
```

Потеря ключа = невозможность обновлять приложение у пользователей.

## Платформы — особенности

- **Иконки.** Исходники — SVG в `crates/app/icons/src/`; `mise run icons`
  (`scripts/icons.sh`, нужны `rsvg-convert` и `magick`) собирает PNG/ICO/ICNS
  двух каналов в `crates/app/icons/prod/` и `dev/` и `public/favicon.svg`;
  PNG коммитятся, CI ничего не рисует. `tauri.conf.json` указывает на `dev/`,
  оверлей `cargo xtask tauri-config` / `build-windows.ps1` — на папку канала
  (`Channel::icons` в `channel.rs`). 32 px и меньше — из `*-small.svg` (без задних
  рёбер куба), у dev — оранжевая полоса DEV.

- **Android.** Gradle-проект (`crates/app/gen/android`) генерируется в CI
  (`tauri android init`) и не коммитится; адаптивная иконка (фон, передний
  план, монохромный слой) — `tauri icon` по манифесту
  `crates/app/icons/<канал>/android/manifest.json`.
  Идентификатор — `mwt.app` в релизе, `dev.mwt.app` в dev
  (`crates/app/tauri.android.conf.json`), десктопный — `mwt.desktop` /
  `dev.mwt.desktop` (от него зависит путь к vault); см. «Каналы».
  Трей, автозапуск, глобальный хоткей, сохранение геометрии окна и системный
  keyring — только десктоп (`cfg(desktop)`): на Android автозапуск всегда
  выключен, задать хоткей нельзя (`UNSUPPORTED`), напоминания заблокированного
  vault живут только в памяти. Пункты автозапуска и хоткея в настройках
  пока не скрыты на телефоне — их переключение там вернёт ошибку.
  Локально: `pnpm tauri android init` → `pnpm tauri android dev`
  (нужны Android SDK, NDK и JDK 17, переменные `ANDROID_HOME`, `NDK_HOME`).
- **Flatpak.** ID `mwt.app.MultiWalletTracker` в релизе, `dev.mwt.MultiWalletTracker`
  в dev (ID на `.desktop` Flatpak не любит, нужно ≥ 3 частей).
  Данные — в `~/.var/app/<ID>/`. Автозапуск из sandbox не
  работает. Для Flathub нужен отдельный PR в flathub с этим манифестом.
- **AUR / pacman.** Пакет `multi-wallet-tracker-bin` ставит готовый бинарник из
  GitHub Releases, поэтому репозиторий должен быть публичным. Шаблон — `packaging/aur/PKGBUILD.in`, рендер —
  `packaging/aur/render.sh`.
- **macOS.** Универсальная сборка (`--target universal-apple-darwin`),
  `scripts/ci/build-macos.sh`. Без сертификата Developer ID приложение
  подписано ad hoc и не нотаризовано: Gatekeeper при первом запуске
  попросит подтвердить (правый клик → «Открыть» или
  `xattr -cr "/Applications/Multi Wallet Tracker.app"`). Собрать macOS на
  другой ОС tauri не умеет. Автозапуск — LaunchAgent в
  `~/Library/LaunchAgents`; при переносе `.app` в корзину он остаётся (хука
  удаления на macOS нет), но запустить уже нечего — выключайте автозапуск в
  настройках до удаления.
- **Автозапуск после удаления.** Запись автозапуска живёт в профиле
  пользователя, поэтому удаление её снимает отдельно: deb/rpm —
  `packaging/linux/postrm.sh`, pacman/AUR — `post_remove` в
  `packaging/aur/multi-wallet-tracker-bin.install` (удаляют записи, что
  запускают `/usr/bin/multi-wallet-tracker`, у всех пользователей), NSIS —
  встроенный шаг деинсталлятора tauri, MSI — фрагмент
  `crates/app/windows/autostart-cleanup.wxs`. На Linux запись ещё и несёт
  `TryExec=<путь к программе>` (`crates/app/src/autostart.rs`): сеанс
  пропускает её, если файла нет, — так и AppImage, удалённый руками, не
  запускается. Обновление записи не трогает.
- **Windows.** Portable-`.exe` и установщики требуют WebView2 Runtime (есть в
  Windows 10/11; NSIS при отсутствии скачивает его). Подписи кода нет —
  SmartScreen будет предупреждать.

## Локально

### Компиляция на всех платформах: `mise run check-platforms`

Быстрая проверка без бандлов (минуты, а не час): `cargo clippy -D warnings`
под linux, windows (`cargo xwin`, `x86_64-pc-windows-msvc`), android
(`aarch64-linux-android` — NDK хоста, иначе образ `ci-android` через podman с
теми же томами, что у `build:all`) и macos (только на Mac или через
`MACOS_BUILD_HOST`). Обязательна после последней правки перед влитием и
коммитом (AGENTS.md, «Сборка на всех платформах»). Логи —
`.cache/xtask/<задача>.log` (`clippy` = linux, `platform-windows.log` …); запускает `cargo xtask check-platforms` (те же задачи входят в `mise run check`); `SKIP` с причиной — машина
платформу проверить не может, `FAIL` — код не собирается.

```bash
mise run check-platforms                    # всё
mise run check-platforms -- windows android # выборочно
```

### Все платформы: `mise run build:all`

`cargo xtask build` собирает бандлы тем же кодом, что CI
(`scripts/ci/build-*.sh`), в тех же образах — на машине нужен только podman
(или docker: `CONTAINER_ENGINE=docker`). Канал — `--channel dev` (по
умолчанию) или `prod` (см. «Каналы»). Результат — в
`.cache/build-all/builds/<канал>/`, лог каждой задачи — в
`.cache/xtask/build-<платформа>.log`, в конце — таблица OK/SKIP/FAIL/CANCELLED
со временем и список файлов. Граф и поведение те же, что у `mise run check`:
первая ошибка гасит остальные сборки (контейнеры останавливаются), второй
такой же запуск подключается к первому, другая команда ждёт его конца.

```bash
mise run build:all                         # всё
mise run build:all -- linux android        # выборочно
mise run build:all -- --version 0.2.0-1 windows
mise run build:all -- --channel prod       # релизное приложение (mwt.*)
mise run build:all -- --heavy 1            # одна тяжёлая сборка за раз (по умолчанию 2)
mise run build:all -- --list linux pacman  # граф задач без запуска
```

Граф: `build:frontend` (`pnpm build` на хосте, один раз, в `dist/`) →
`android`, `windows`, `linux` (тяжёлые, не больше `--heavy` за раз) →
`pacman`, `flatpak` (после `linux`); `macos` не ждёт никого. Платформенные
`tauri build` не запускают фронт повторно: оверлей `cargo xtask tauri-config
--prebuilt-frontend` содержит пустой `beforeBuildCommand` (CI флаг не задаёт). `pacman` и `flatpak` без `linux` в списке
берут tarball из папки результата, а если его там нет — ошибка до начала
сборки («build it first»).

| Платформа | Где собирается | Что получится |
|---|---|---|
| `linux` | образ `ci-linux` (`--native` — на хосте) | `.deb`, `.rpm`, `.AppImage`, `tar.gz` |
| `pacman` | `archlinux:base-devel`, после `linux` | `.pkg.tar.zst` |
| `flatpak` | `archlinux:base-devel` `--privileged`, после `linux` | `.flatpak` |
| `android` | образ `ci-android` | APK и AAB; без `ANDROID_KEYSTORE_*` — тестовый ключ, суффикс `-testkey` |
| `windows` | `ci-linux` + `cargo-xwin` (`scripts/local/windows-cross.Dockerfile`) | NSIS `-setup.exe`, portable `.zip`; MSI нет — WiX работает только на Windows, его делает CI |
| `macos` | на Mac — сразу; с Linux — на `MACOS_BUILD_HOST=user@mac` по ssh (rsync туда и обратно, фронт собирает сам Mac) | `.dmg`, `.app.tar.gz`; без Mac — SKIP |

- Образы берутся из ghcr.io (`ghcr.io/reckeck/mwt/ci-<вид>:<тег>`, тег — `mise run
  ci-image-tag`; другой реестр — `MWT_CI_REGISTRY`), если их там нет — собираются из
  `scripts/ci/ci.Dockerfile`.
- Сборочные каталоги контейнеров — тома `mwt-build-<хэш пути копии>-*`
  (свои у каждой копии: cargo сверяет время файлов, и чужие артефакты
  выглядели бы свежими; `.rust-target`,
  `node_modules`, `/root`): хостовые `.rust-target` и `node_modules` не
  трогаются, повторная сборка идёт инкрементально. Начать с нуля —
  `podman volume rm $(podman volume ls -q | grep mwt-build-)`.
- Сборки идут рядом, поэтому у каждой свой overlay tauri
  (`.cache/tauri-ci-<платформа>.json`, переменная `TAURI_CI_CONFIG`), а Android
  работает на личной копии `crates/app/icons` (`MWT_ICONS_ISOLATED=1`): `tauri
  icon` переписывает иконки, пока linux и windows их читают.
- Версия по умолчанию — из `tauri.conf.json` (`cargo xtask version`).
- Первый `windows` дольше: строится образ с `cargo-xwin`, а тот скачивает
  Windows SDK (кэш в томе `mwt-build-<копия>-windows-home`).

### По частям

```bash
mise run build                                       # deb/rpm/AppImage
scripts/ci/collect-linux.sh "$(cargo xtask version)" release
packaging/aur/render.sh 0.1.0 release/multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz > PKGBUILD
```
