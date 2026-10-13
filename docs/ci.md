# CI/CD (GitLab) и платформы

Пайплайн описан в `.gitlab-ci.yml`, вся логика — в `scripts/ci/*`, манифесты
упаковки — в `packaging/`. Версии Rust/Node/pnpm берутся из `mise.toml`
(на Linux через сам mise, на Windows скрипт читает их из файла).

## Стадии

| Стадия | Job | Что делает |
|---|---|---|
| check | `frontend:check` | `mise run lint`, `typecheck`, `check-onboarding`, `check-forms` |
| check | `rust:check` | `cargo fmt --check`, `mise run clippy`, `mise run test`, `gen-types` + проверка, что `ipc.gen.d.ts` закоммичен актуальным |
| check | `version:check` | только на теге: `vX.Y.Z` = версия в `tauri.conf.json` и `Cargo.toml` |
| check | `sast`, `secret_detection` | шаблоны GitLab Security |
| build | `build:linux` | `.deb`, `.rpm`, `.AppImage`, `tar.gz` с бинарником (Ubuntu 22.04 — старый glibc, файлы запускаются на новых дистрибутивах) |
| build | `build:windows` | NSIS-установщик `.exe`, MSI (по одному на `en-US` и `ru-RU`), portable `.zip` с голым `.exe` |
| build | `build:android` | универсальные APK и AAB (`aarch64`, `armv7`, `x86_64`), подписанные |
| package | `package:flatpak` | `.flatpak`-бандл (рантайм GNOME 50) из Linux-tarball |
| package | `package:pacman` | `multi-wallet-tracker-bin-*.pkg.tar.zst` из того же tarball |
| release | `release` | только на теге: всё из `release/` + `SHA256SUMS` → Generic Package Registry, GitLab Release со ссылками и заметками из `CHANGELOG.md` |
| release | `aur:publish` | вручную, на теге при заданном `AUR_SSH_PRIVATE_KEY`: пуш `PKGBUILD` + `.SRCINFO` в AUR |

Сборки (`build`, `package`) идут автоматически на тегах, на `main`, при ручном
запуске и по расписанию; в MR — по кнопке (manual, не блокируют MR).

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
- Android-сборка длинная (release + LTO на три архитектуры): timeout 3 ч.
  Меньше архитектур — `ANDROID_TARGETS: aarch64`.

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
