# CI/CD (GitLab) и платформы

Как работать с ветками `feature/*`, `hotfix/*`, `development`, `main` и
выпуском — [workflow.md](workflow.md). Здесь — устройство пайплайна.

Пайплайн описан в `.gitlab-ci.yml`, вся логика — в `scripts/ci/*`, манифесты
упаковки — в `packaging/`. Версии Rust/Node/pnpm берутся из `mise.toml`
(на Linux через сам mise, на Windows скрипт читает их из файла).

## Стадии

| Стадия | Job | Что делает |
|---|---|---|
| .pre | `ci:image` | только когда меняются входы образов: собирает и пушит CI-образы (см. ниже) |
| check | `frontend:check` | `mise run lint`, `typecheck`, `check-onboarding`, `check-forms` |
| check | `rust:fmt` | `cargo fmt --check` — без компиляции, секунды |
| check | `rust:clippy` | `mise run clippy` |
| check | `rust:test` | `mise run test`, `gen-types` + проверка, что `ipc.gen.d.ts` закоммичен актуальным |
| check | `ci:lint` | `scripts/ci/lint-ci.sh`: `*.ps1` только ASCII (Windows PowerShell 5.1), `bash -n` всех скриптов |
| check | `version` | только где есть сборки: версия сборки (`scripts/ci/version.sh`, dotenv `BUILD_VERSION`), стоп, если `vX.Y.Z` уже выпущен |
| check | `sast`, `secret_detection` | шаблоны GitLab Security |
| build | `build:linux` | `.deb`, `.rpm`, `.AppImage`, `tar.gz` с бинарником (Ubuntu 22.04 — старый glibc, файлы запускаются на новых дистрибутивах) |
| build | `build:windows` | NSIS-установщик `.exe`, MSI (по одному на `en-US` и `ru-RU`), portable `.zip` с голым `.exe` |
| build | `build:android` | универсальные APK и AAB (`aarch64`, `armv7`, `x86_64`), подписанные |
| package | `package:flatpak` | `.flatpak`-бандл (рантайм GNOME 50) из Linux-tarball |
| package | `package:pacman` | `multi-wallet-tracker-bin-*.pkg.tar.zst` из того же tarball |
| release | `prerelease` | `development`: всё из `release/` + `SHA256SUMS` → Package Registry, Release «Pre-release X.Y.Z-N» (тег создаёт API) |
| release | `prerelease:cleanup` | `development`: оставить последние `PRERELEASES_KEPT` пре-релизов (нужен `RELEASE_TOKEN`) |
| release | `release` | `main`: то же для `X.Y.Z`, заметки из раздела `CHANGELOG.md` |
| release | `backmerge` | `main`: MR `main` → `development` после выпуска (нужен `RELEASE_TOKEN`) |
| release | `aur:publish` | `main`, вручную, при заданном `AUR_SSH_PRIVATE_KEY`: пуш `PKGBUILD` + `.SRCINFO` в AUR |

Сборки (`build`, `package`) идут в `development`, `main`, при ручном запуске
(Run pipeline) и по расписанию — и только когда **все** проверки зелёные.
Фиче-ветки, хотфиксы и MR — только проверки. Ручной запуск и расписание
ничего не публикуют. Теговых пайплайнов нет: теги создают job'ы выпуска.

## Скорость

Что делает пайплайн быстрым и что не надо ломать:

- **Готовые образы.** Job'ы идут в образах из `scripts/ci/ci.Dockerfile`
  (GitLab Container Registry проекта): `ci-node` (mise + node/pnpm,
  фронтенд), `ci-linux` (+ библиотеки Tauri, Rust, sccache), `ci-android`
  (+ JDK 17, SDK/NDK, Rust-таргеты). Job не ставит ни apt-пакеты, ни
  тулчейн — `setup-linux.sh` только подключает кэши. Тег образа —
  `CI_IMAGE_TAG`, хеш входов (Dockerfile, `apt-packages.sh`, `[tools]` из
  `mise.toml`): поменял что-то из этого — обнови тег
  (`scripts/ci/image-tag.sh` печатает новый, `ci:image` без этого упадёт и
  подскажет). Сборка образов идёт только в пайплайне, где входы поменялись;
  уже запушенный тег не пересобирается (`FORCE_CI_IMAGE=1` — принудительно).
  Без образов (`CI_NODE_IMAGE`/`CI_LINUX_IMAGE`/`CI_ANDROID_IMAGE` =
  `ubuntu:22.04`) всё ставится скриптом — медленно, но работает.
- **sccache.** Скомпилированные крейты лежат в GitLab-кэше
  (`.cache/sccache`, ключ на job и `CI_IMAGE_TAG`) и возвращаются через
  `RUSTC_WRAPPER=sccache`, в том числе на Windows. Скачанные крейты —
  `.cache/cargo-registry`.
- **Кэш пишет только `main`.** В MR, ветках и тегах кэш только читается
  (`CACHE_POLICY`), поэтому они не тратят время на выгрузку.
- **Тесты быстрые.** KDF vault (Argon2id, 64 MiB × 3) идёт на каждом
  сохранении; `argon2`/`blake2` собираются с `opt-level = 3` даже в debug
  (`Cargo.toml`), иначе `cargo test` шёл ~5,5 мин вместо ~30 с. Параметры
  KDF не менялись.
- **Лишнее не запускается.** В MR и ветках job'ы Rust идут, только если
  менялись `crates/`, `Cargo.*`, `ipc.gen.d.ts`, CI или `mise.toml`;
  фронтенд — если менялись `src/`, конфиги фронта, `pnpm-lock.yaml`. На
  `main` и тегах идёт всё.
- **Новый пуш отменяет старый пайплайн** (`interruptible` +
  `auto_cancel: on_new_commit`); выпуск и AUR не прерываются.
- **Кэш пишут `development` и `main`**, остальные читают; ключи
  постоянные (не от хеша lock-файлов), так что смена зависимостей стартует
  с прошлого кэша. Чтобы фиче-ветки видели кэш защищённых веток, в GitLab
  выключено «Use separate caches for protected branches» (см.
  [workflow.md](workflow.md)). `AccessDenied … (or it may not exist)` от
  `storage.googleapis.com/gitlab-com-runners-cache` — промах кэша, не ошибка.
- **Большие раннеры для компиляции.** `rust:clippy`, `rust:test`,
  `build:linux`, `build:android` идут на `CI_HEAVY_RUNNER_TAG`
  (`saas-linux-medium-amd64`, 4 vCPU; лёгкие job'ы — на small, 2 vCPU).
  Быстрее — `saas-linux-large-amd64` (8 vCPU: `build:linux` 433 с против 1113 с
  на small), дешевле — `saas-linux-small-amd64`.
- **Пре-релизы — thin LTO.** В `development` (и ручных/плановых пайплайнах)
  `CARGO_PROFILE_RELEASE_LTO=thin` и `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`:
  финальная оптимизация идёт параллельно, а не в один поток. Релизы из
  `main` собираются с полным LTO и одной единицей (`[profile.release]` в
  `Cargo.toml`) — самый маленький и быстрый бинарник.
- Быстрое сжатие кэша и артефактов (`FF_USE_FASTZIP`), без debug-info в
  проверках (`CARGO_PROFILE_DEV_DEBUG=0`).

## Выпуск версии

Пре-релизы — каждый пуш в `development`, релиз — мерж в `main`; шаги — в
[workflow.md](workflow.md#выпуск). Версия сборки:
`scripts/ci/version.sh --channel dev|prod|none` (dev и ручные — `X.Y.Z-N`,
prod — `X.Y.Z`); в сборку она попадает через `tauri build --config`
(`scripts/ci/tauri-config.sh`), вместе с Android `versionCode` = номер
пайплайна.

## Раннеры

- Linux-job'ы — любые docker-раннеры. `package:flatpak` нужен
  **privileged** docker (bubblewrap); общие раннеры GitLab.com такие.
- `build:windows` — Windows-раннер с тегом из `WINDOWS_RUNNER_TAG`
  (по умолчанию `saas-windows-medium-amd64`, общий раннер GitLab.com; для своего
  раннера — его тег). MSVC Build Tools ставятся через choco, если их нет.
- Android-сборка длинная (release + LTO, три архитектуры —
  `ANDROID_TARGETS`): timeout 2 ч.
- `ci:image` — docker-in-docker (`docker:27-dind`), нужен privileged-раннер
  и включённый Container Registry проекта.

## CI/CD-переменные (Settings → CI/CD → Variables)

| Переменная | Нужна для | Примечание |
|---|---|---|
| `ANDROID_KEYSTORE_BASE64` | релизная подпись APK/AAB | `base64 -w0 release.keystore`; masked |
| `ANDROID_KEYSTORE_PASSWORD` | то же | masked |
| `ANDROID_KEY_ALIAS` | то же | |
| `ANDROID_KEY_PASSWORD` | то же | по умолчанию = пароль хранилища |
| `AUR_SSH_PRIVATE_KEY` | `aur:publish` | тип **File**; ключ, добавленный в аккаунт AUR |
| `RELEASE_TOKEN` | `prerelease:cleanup`, `backmerge`; `release`, если `CI_JOB_TOKEN` не хватает прав | project access token, роль Maintainer, scope `api` |

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

- **Android.** Gradle-проект (`crates/app/gen/android`) генерируется в CI
  (`tauri android init`) и не коммитится; иконки — `tauri icon`.
  Идентификатор — `dev.mwt.app` (`crates/app/tauri.android.conf.json`),
  десктопный — `dev.mwt.desktop` (от него зависит путь к vault).
  Трей, автозапуск, глобальный хоткей, сохранение геометрии окна и системный
  keyring — только десктоп (`cfg(desktop)`): на Android автозапуск всегда
  выключен, задать хоткей нельзя (`UNSUPPORTED`), напоминания заблокированного
  vault живут только в памяти. Пункты автозапуска и хоткея в настройках
  пока не скрыты на телефоне — их переключение там вернёт ошибку.
  Локально: `pnpm tauri android init` → `pnpm tauri android dev`
  (нужны Android SDK, NDK и JDK 17, переменные `ANDROID_HOME`, `NDK_HOME`).
- **Flatpak.** ID `dev.mwt.MultiWalletTracker` (ID на `.desktop` Flatpak не любит).
  Данные — в `~/.var/app/dev.mwt.MultiWalletTracker/`. Автозапуск из sandbox не
  работает. Для Flathub нужен отдельный PR в flathub с этим манифестом.
- **AUR / pacman.** Пакет `multi-wallet-tracker-bin` ставит готовый бинарник из
  Package Registry, поэтому проект (или хотя бы его пакеты) должен быть
  публичным. Шаблон — `packaging/aur/PKGBUILD.in`, рендер —
  `packaging/aur/render.sh`.
- **Windows.** Portable-`.exe` и установщики требуют WebView2 Runtime (есть в
  Windows 10/11; NSIS при отсутствии скачивает его). Подписи кода нет —
  SmartScreen будет предупреждать.

## Локально

```bash
mise run build                                       # deb/rpm/AppImage
scripts/ci/collect-linux.sh "$(scripts/ci/version.sh)" release
packaging/aur/render.sh 0.1.0 release/multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz > PKGBUILD
```
