# CI/CD (GitLab) и платформы

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
| check | `version:check` | только на теге: `vX.Y.Z` = версия в `tauri.conf.json` и `Cargo.toml` |
| check | `sast`, `secret_detection` | шаблоны GitLab Security |
| build | `build:linux` | `.deb`, `.rpm`, `.AppImage`, `tar.gz` с бинарником (Ubuntu 22.04 — старый glibc, файлы запускаются на новых дистрибутивах) |
| build | `build:windows` | NSIS-установщик `.exe`, MSI (по одному на `en-US` и `ru-RU`), portable `.zip` с голым `.exe` |
| build | `build:android` | универсальные APK и AAB (`aarch64`, `armv7`, `x86_64`), подписанные |
| package | `package:flatpak` | `.flatpak`-бандл (рантайм GNOME 50) из Linux-tarball |
| package | `package:pacman` | `multi-wallet-tracker-bin-*.pkg.tar.zst` из того же tarball |
| release | `release` | только на теге и только когда **все** проверки и сборки зелёные: всё из `release/` + `SHA256SUMS` → Generic Package Registry, GitLab Release со ссылками и заметками из `CHANGELOG.md` |
| release | `aur:publish` | вручную, на теге при заданном `AUR_SSH_PRIVATE_KEY`: пуш `PKGBUILD` + `.SRCINFO` в AUR |

Сборки (`build`, `package`) идут автоматически на тегах, на `main`, при ручном
запуске и по расписанию; в MR — по кнопке (manual, не блокируют MR). Они
стартуют сразу, параллельно с проверками, а не после них; выпуск при этом
всё равно ждёт проверки.

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
- **Android вне тега — только `aarch64`** (телефоны); на теге — `aarch64`,
  `armv7`, `x86_64`.
- Быстрое сжатие кэша и артефактов (`FF_USE_FASTZIP`), без debug-info в
  проверках (`CARGO_PROFILE_DEV_DEBUG=0`).

## Выпуск версии

1. Поднять версию в `crates/app/tauri.conf.json` и `[workspace.package]` в
   `Cargo.toml` (и `<release>` в `packaging/flatpak/*.metainfo.xml`), раздел
   `## [X.Y.Z]` в `CHANGELOG.md`.
2. Поставить тег `vX.Y.Z` и запушить его (`jj git push` + `git push origin vX.Y.Z`).
3. Пайплайн соберёт всё и создаст Release. AUR — кнопкой `aur:publish`.

## Раннеры

- Linux-job'ы — любые docker-раннеры. `package:flatpak` нужен
  **privileged** docker (bubblewrap); общие раннеры GitLab.com такие.
- `build:windows` — Windows-раннер с тегом из `WINDOWS_RUNNER_TAG`
  (по умолчанию `saas-windows-medium-amd64`, общий раннер GitLab.com; для своего
  раннера — его тег). MSVC Build Tools ставятся через choco, если их нет.
- Android-сборка длинная (release + LTO): timeout 2 ч; на теге три
  архитектуры, иначе одна (`ANDROID_TARGETS`).
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
| `RELEASE_TOKEN` | `release`, если `CI_JOB_TOKEN` не хватает прав | project access token со scope `api` |

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
