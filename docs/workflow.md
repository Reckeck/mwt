# Как работать: ветки, пре-релизы, релизы

Как код доходит от задачи до пользователя и что делает CI на каждом шаге.
Устройство самого пайплайна (образы, кэши, переменные) — в [ci.md](ci.md).

## Ветки

| Ветка | Откуда | Куда сливается | Что делает CI |
|---|---|---|---|
| `feature/<задача>` | от `development` | MR в `development` | только проверки, и только те, чьи файлы задеты |
| `hotfix/<задача>` | от `main` | MR в `main` | только проверки |
| `development` | — | MR в `main` (выпуск) | проверки → сборки всех платформ → **пре-релиз** `X.Y.Z-N` |
| `main` | — | — | проверки → сборки → **релиз** `X.Y.Z` → MR обратно в `development` |

- В `development` и `main` — только через MR, без прямых пушей и
  force-push (защищённые ветки).
- Сборки начинаются, только когда **все** проверки зелёные; красная проверка
  останавливает пайплайн до сборок.
- Теги руками не ставятся: их создаёт job выпуска вместе с GitLab Release.
  Теговых пайплайнов нет.

## Версии

Версия — `X.Y.Z` в `crates/app/tauri.conf.json`, `[workspace.package]`
`Cargo.toml`, `Cargo.lock` и `<release>` в
`packaging/flatpak/*.metainfo.xml`. Руками её не меняют: это делает
`mise run bump` (см. «Выпуск») вместе с разделом `CHANGELOG.md`.

- **Пре-релиз** из `development`: `X.Y.Z-N`, где `N` — номер пайплайна
  (только цифры — так требует MSI). Если `X.Y.Z` уже выпущен, а bump ещё не
  было, пре-релиз сам становится `X.Y.(Z+1)-N`. Для pacman это `X.Y.ZpreN` —
  идёт раньше релиза `X.Y.Z` и обновится на него.
- **Релиз** из `main`: ровно `X.Y.Z`, тег `vX.Y.Z`. Job `version`
  останавливает пайплайн, если `vX.Y.Z` уже выпущен или в `CHANGELOG.md`
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
- Заметки пре-релиза в GitLab пишет CI сам — коммиты с прошлого релиза.

Обе команды работают в основной (colocated) копии: им нужна git-история,
у side-workspace jj её нет.
- Android `versionCode` — номер пайплайна: растёт всегда, так что любая
  следующая сборка ставится поверх предыдущей.

## Задача: от начала до пре-релиза

```bash
jj git fetch
jj new development@origin                 # новая работа поверх development
# … правки, `mise run check` …
jj describe -m "feat(…): …"               # контекстные коммиты, см. AGENTS.md
jj bookmark create feature/<задача> -r @
jj git push -b feature/<задача>           # пайплайн: только проверки
glab mr create --source-branch feature/<задача> --target-branch development --fill
```

MR зелёный и проверен → merge → пайплайн `development` собирает всё и
публикует пре-релиз (Releases → «Pre-release X.Y.Z-N», файлы в Package
Registry). Последние `PRERELEASES_KEPT` (10) пре-релизов хранятся, старые
удаляются (`prerelease:cleanup`, нужен `RELEASE_TOKEN`).

## Выпуск

1. В `development` (основная копия): `mise run bump` — версия и раздел
   CHANGELOG; при желании дописать над списком коммитов главное словами;
   `mise run check`; коммит `chore(release): vX.Y.Z`, пуш, MR в
   `development` (или прямо в составе MR выпуска).
2. MR `development` → `main`, merge.
3. Пайплайн `main`: проверки → сборки → Release `vX.Y.Z` с файлами и
   заметками из CHANGELOG. AUR — кнопкой `aur:publish`.
4. `backmerge` открывает MR `main` → `development` (нужен `RELEASE_TOKEN`) —
   слить его. Поднимать версию сразу не нужно: пре-релизы до следующего
   bump сами идут как `X.Y.(Z+1)-N`.

## Хотфикс

```bash
jj new main@origin
# … исправление (коммит fix: …), затем `mise run bump patch` …
jj bookmark create hotfix/<задача> -r @ && jj git push -b hotfix/<задача>
glab mr create --source-branch hotfix/<задача> --target-branch main --fill
```

Merge → релиз `vX.Y.(Z+1)` → `backmerge` приносит исправление в
`development`.

## Следить за пайплайном

```bash
glab ci status                 # пайплайн текущей ветки
glab ci view                   # интерактивно: job'ы, логи, повтор
glab ci trace <job>            # лог job'а
glab mr list                   # открытые MR
```

Скилл `glab` (`.agents/skills/glab`) — как этим пользоваться; правка
`.gitlab-ci.yml` — через скиллы `gitlab-ci-generator` и
`gitlab-ci-validator` (валидатор:
`bash .agents/skills/gitlab-ci-validator/scripts/validate_gitlab_ci.sh .gitlab-ci.yml`).

## Разовая настройка GitLab

1. **Ветка `development`** от `main`, сделать её веткой по умолчанию
   (Settings → Repository → Branch defaults): MR по умолчанию идут туда, а
   `rules: changes` в CI сравнивают с ней — без неё пайплайны фиче-веток
   не создаются.
2. **Защита веток** (Settings → Repository → Protected branches): `main` и
   `development` — Allowed to merge: Maintainers, Allowed to push: No one,
   force push выключен.
3. **Кэш для всех веток** (Settings → CI/CD → General pipelines): выключить
   «Use separate caches for protected branches». Иначе кэш, который пишут
   `development` и `main` (ключи `…-protected`), не виден фиче-веткам и MR,
   и они каждый раз собирают с нуля. Сообщение `AccessDenied … (or it may not
   exist)` от `storage.googleapis.com/gitlab-com-runners-cache` в логе —
   это промах кэша (такого объекта нет), не ошибка: первый прогон после
   смены ключа всегда такой.
4. **Переменные** (Settings → CI/CD → Variables): `RELEASE_TOKEN` (project
   access token, роль Maintainer, scope `api`, masked) — удаление старых
   пре-релизов и back-merge MR; ключи подписи Android и `AUR_SSH_PRIVATE_KEY`
   — см. [ci.md](ci.md).
5. **Container Registry** включён (CI-образы).
