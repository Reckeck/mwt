---
name: commit-coauthor
description: >
  Как агент помечает свои коммиты трейлером Co-authored-by с указанием себя
  (инструмента: OpenCode, Claude Code, Codex, Zed…). Использовать перед
  каждым `jj describe` / `jj commit` / `jj new -m`, при переписывании
  сообщения чужого коммита и при вопросах про авторство агента. Триггеры:
  коммит, commit, describe, соавтор, co-author, Co-authored-by, трейлер,
  trailer, авторство, «кто написал».
---

# Соавтор в коммитах: агент указывает себя сам

jj **не** добавляет трейлер автоматически (`templates.commit_trailers` в
repo-конфиге не задан). Каждый агент, который пишет сообщение коммита,
**сам** дописывает строку соавторства с **собственным** именем — именем
инструмента, которым он работает, а не чужим.

## Формат

Последний абзац сообщения, отделён пустой строкой:

```
feat(web): build the dashboard from its widgets

Body: what changed and why.

Co-authored-by: <Инструмент> <email>
```

| Инструмент | Строка |
|---|---|
| OpenCode | `Co-authored-by: OpenCode <agent@opencode.local>` |
| Claude Code | `Co-authored-by: Claude Code <noreply@anthropic.com>` |
| Codex | `Co-authored-by: Codex <noreply@openai.com>` |
| Zed | `Co-authored-by: Zed Agent <agent@zed.dev>` |

- Пиши **своё** имя. Если твоего инструмента нет в таблице — добавь строку
  `Co-authored-by: <Имя инструмента> <email>` и дополни таблицу этим же
  изменением.
- Если твоя среда задаёт свою строку атрибуции (например, системная
  инструкция с точным текстом `Co-Authored-By: …`) — используй её дословно,
  она имеет приоритет над таблицей.
- Ровно **одна** строка на инструмент. Два агента работали над change —
  две строки, по одной от каждого.

## Как писать с jj

```bash
jj describe -m "fix(core): keep vault unlock error as Error::Secret

Why it changed.

Co-authored-by: Claude Code <noreply@anthropic.com>"
```

То же для `jj commit -m` и `jj new -m`. Проверка:

```bash
jj log -r @ --no-graph -T description | tail -3   # твоя строка на месте, дубля нет
```

## Правила

- **Не убирай чужие трейлеры.** Переписывая сообщение (`jj describe -r …`,
  `jj squash`, `jj split`) — сохрани все существующие `Co-authored-by:` и
  добавь свою, если её ещё нет.
- **Не дублируй.** Перед `describe` посмотри текущее сообщение; если твоя
  строка уже есть — не добавляй вторую.
- **Автоподстановка выключена намеренно.** Если
  `jj config list templates.commit_trailers` что-то выводит — шаблон снова
  включён и будет подставлять чужое имя рядом с твоим. Не включай его и
  сообщи пользователю, если нашёл.
- Коммит — по-прежнему **только по явной просьбе пользователя** (скилл
  `jj-workflow`).
- GitHub показывает трейлер `Co-authored-by` аватаром соавтора у коммита, только
  если email принадлежит аккаунту; `*.local` / `noreply` без аккаунта — нет, но в
  тексте коммита трейлер остаётся.
