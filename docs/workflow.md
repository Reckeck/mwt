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

Версия — `X.Y.Z` в `crates/app/tauri.conf.json` и `[workspace.package]`
`Cargo.toml` (одинаковая; `<release>` в
`packaging/flatpak/*.metainfo.xml` — тоже). В ветке всегда «следующая
выпускаемая» версия.

- **Пре-релиз** из `development`: `X.Y.Z-N`, где `N` — номер пайплайна
  (только цифры — так требует MSI). Это сборка будущего `X.Y.Z`: для pacman
  она `X.Y.ZpreN` и идёт раньше релиза `X.Y.Z` (обновится на него).
- **Релиз** из `main`: ровно `X.Y.Z`, тег `vX.Y.Z`.
- Если `vX.Y.Z` уже выпущен, job `version` останавливает пайплайн: на `main` —
  «подними версию перед мержем», на `development` — «подними до следующей».
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

1. В `development`: версия `X.Y.Z` (уже выставлена с прошлого выпуска),
   раздел `## [X.Y.Z]` в `CHANGELOG.md` вместо `[Unreleased]`.
2. MR `development` → `main`, merge.
3. Пайплайн `main`: проверки → сборки → Release `vX.Y.Z` с файлами и
   заметками из CHANGELOG. AUR — кнопкой `aur:publish`.
4. `backmerge` открывает MR `main` → `development` (нужен `RELEASE_TOKEN`) —
   слить его и поднять версию в `development` до следующей (`X.Y.(Z+1)` или
   `X.(Y+1).0`), иначе следующие пре-релизы остановит `version`.

## Хотфикс

```bash
jj new main@origin
# … исправление, версия X.Y.(Z+1) в Cargo.toml + tauri.conf.json, CHANGELOG …
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
