# Как работать: ветки, пре-релизы, релизы

Как код доходит от задачи до пользователя и что делает CI на каждом шаге.
Устройство самого CI (workflows, образы, кэши, секреты) — в [ci.md](ci.md).

## Ветки

| Ветка | Откуда | Куда сливается | Что делает CI |
|---|---|---|---|
| `feature/<задача>` | от `development` | PR в `development` | только проверки |
| `hotfix/<задача>` | от `main` | PR в `main` | только проверки |
| `development` | — | PR в `main` (выпуск) | проверки → сборки всех платформ → **пре-релиз** `X.Y.Z-N` |
| `main` | — | — | проверки → сборки → **релиз** `X.Y.Z` → PR обратно в `development` |

- В `development` и `main` — только через PR, без прямых пушей и
  force-push (защищённые ветки).
- Сборки начинаются, только когда **все** проверки зелёные; красная проверка
  отменяет прогон `release.yml` до публикации.
- Теги руками не ставятся: их создаёт job выпуска вместе с GitHub Release.

## Версии

Версия — `X.Y.Z` в `crates/app/tauri.conf.json`, `[workspace.package]`
`Cargo.toml`, `Cargo.lock` и `<release>` в
`packaging/flatpak/*.metainfo.xml`. Руками её не меняют: это делает
`mise run bump` (см. «Выпуск») вместе с разделом `CHANGELOG.md`.

- **Пре-релиз** из `development`: `X.Y.Z-N`, где `N` — последний
  опубликованный `vX.Y.Z-N` того же `X.Y.Z` плюс один, иначе 1 (только цифры — так требует MSI). Если `X.Y.Z` уже выпущен, а bump ещё не
  было, пре-релиз сам становится `X.Y.(Z+1)-N`. Для pacman это `X.Y.ZpreN` —
  идёт раньше релиза `X.Y.Z` и обновится на него.
- **Релиз** из `main`: ровно `X.Y.Z`, тег `vX.Y.Z`. Job `version`
  останавливает выпуск, если `vX.Y.Z` уже выпущен или в `CHANGELOG.md`
  нет раздела `## [X.Y.Z]` — значит, на `development` забыли `mise run bump`.

## CHANGELOG

`CHANGELOG.md` собирается из коммитов: они в формате Conventional Commits
(`feat`, `fix`, `perf`, … — см. AGENTS.md), `git-cliff` (`cliff.toml`)
раскладывает их по группам «Добавлено», «Исправлено», «Быстрее»…

- `mise run changelog` — обновить список коммитов в `## [Unreleased]`
  (между `<!-- commits:start/end -->`). Над ним можно писать руками
  главное для пользователя — этот текст сохраняется.
- `mise run bump` — следующая версия по коммитам с прошлого релиза (`feat` →
  minor, остальное → patch; `mise run bump patch|minor|major|X.Y.Z` —
  явно), во все файлы версии, и `[Unreleased]` становится
  `## [X.Y.Z] — дата`. 1.0 и любой major — только явно.
- Заметки пре-релиза в GitHub Releases пишет CI сам — коммиты с прошлого релиза.

Обе команды работают в основной (colocated) копии: им нужна git-история,
у side-workspace jj её нет.
- Android `versionCode` — номер запуска: растёт всегда, так что любая
  следующая сборка ставится поверх предыдущей.

## Задача: от начала до пре-релиза

```bash
jj git fetch
jj new development@origin                 # новая работа поверх development
# … правки, `mise run check` …
jj describe -m "feat(…): …"               # контекстные коммиты, см. AGENTS.md
jj bookmark create feature/<задача> -r @
jj git push -b feature/<задача>           # CI: только проверки (PR)
gh pr create --base development --head feature/<задача> --fill
```

PR зелёный и проверен → merge → `release.yml` на `development` собирает всё и
публикует пре-релиз (GitHub Releases, `vX.Y.Z-N`, файлы — вложения релиза).
Последние 10 пре-релизов хранятся, старые удаляются
(`cargo xtask prune-prereleases`, нужен `GH_TOKEN`).

## Выпуск

Шаги выпуска — команды `cargo xtask publish`, `prune-prereleases`,
`backmerge`, `publish-aur` (токен `GH_TOKEN`, см. [ci.md](ci.md#выпуск-через-xtask-github)).
Их вызывает `.github/workflows/release.yml` (граф, секреты, ручной запуск —
[ci.md](ci.md#releaseyml)).

1. В `development` (основная копия): `mise run bump` — версия и раздел
   CHANGELOG; при желании дописать над списком коммитов главное словами;
   `mise run check`; коммит `chore(release): vX.Y.Z`, пуш, PR в
   `development` (или прямо в составе PR выпуска).
2. PR `development` → `main`, merge.
3. `release.yml` на `main`: проверки → сборки → Release `vX.Y.Z` с файлами и
   заметками из CHANGELOG (Linux, Windows, Android, Flatpak, pacman; macOS —
   пока на паузе). AUR — job `aur` (нужен секрет `AUR_SSH_PRIVATE_KEY`). Проверить
   бандлы до мержа можно и локально: `mise run build:all` (`docs/ci.md`).
4. `backmerge` открывает PR `main` → `development` (`cargo xtask backmerge`, нужен `GH_TOKEN`) —
   слить его. Поднимать версию сразу не нужно: пре-релизы до следующего
   bump сами идут как `X.Y.(Z+1)-N`.

## Хотфикс

```bash
jj new main@origin
# … исправление (коммит fix: …), затем `mise run bump patch` …
jj bookmark create hotfix/<задача> -r @ && jj git push -b hotfix/<задача>
gh pr create --base main --head hotfix/<задача> --fill
```

Merge → релиз `vX.Y.(Z+1)` → `backmerge` приносит исправление в
`development`.

## Следить за CI

```bash
gh pr checks                   # проверки PR текущей ветки
gh run list --branch <ветка>   # запуски workflows ветки
gh run view <id> --log-failed  # лог упавших шагов
gh run watch <id>              # ждать конца запуска
gh pr list                     # открытые PR
```

Проверка самих workflow-файлов — `mise x actionlint@1.7.12 -- actionlint`.

## Разовая настройка GitHub

1. **Ветка `development`** от `main`, сделать её веткой по умолчанию
   (Settings → General → Default branch): PR по умолчанию идут туда, а
   кэши Actions видны веткам от неё.
2. **Защита веток** (Settings → Branches): `main` и `development` — только
   через PR, обязательная проверка `ci-ok`, без force-push.
3. **Автоудаление веток** (Settings → General → «Automatically delete head
   branches»): `cache-cleanup.yml` тогда чистит кэши влитых веток.
4. **Секреты и права** (Settings → Secrets and variables → Actions):
   ключи подписи Android, `AUR_SSH_PRIVATE_KEY`, при необходимости
   `BACKMERGE_TOKEN` — см. [ci.md](ci.md#секреты-settings--secrets-and-variables--actions);
   включить «Allow GitHub Actions to create and approve pull requests» для
   `backmerge`.
