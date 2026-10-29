---
name: commit-coauthor
description: >
  How an agent marks its own commits with a Co-authored-by trailer naming itself
  (the tool: OpenCode, Claude Code, Codex, Zed...). Use before every
  `jj describe` / `jj commit` / `jj new -m`, when rewriting someone else's
  commit message, and for questions about agent authorship. Triggers:
  commit, коммит, describe, co-author, соавтор, Co-authored-by, trailer, трейлер,
  authorship, авторство, "кто написал" (who wrote it).
---

# Co-author in commits: the agent names itself

jj does **not** add the trailer automatically (`templates.commit_trailers` is not set
in the repo config). Every agent that writes a commit message
appends the co-author line **itself**, with its **own** name: the name of
the tool it works with, not someone else's.

## Format

The last paragraph of the message, separated by a blank line:

```
feat(web): build the dashboard from its widgets

Body: what changed and why.

Co-authored-by: <Tool> <email>
```

| Tool | Line |
|---|---|
| OpenCode | `Co-authored-by: OpenCode <agent@opencode.local>` |
| Claude Code | `Co-authored-by: Claude Code <noreply@anthropic.com>` |
| Codex | `Co-authored-by: Codex <noreply@openai.com>` |
| Zed | `Co-authored-by: Zed Agent <agent@zed.dev>` |

- Write **your own** name. If your tool is not in the table, add the line
  `Co-authored-by: <Tool name> <email>` and extend the table in the same
  change.
- If your environment sets its own attribution line (for example, a system
  instruction with the exact text `Co-Authored-By: …`), use it verbatim;
  it takes priority over the table.
- Exactly **one** line per tool. If two agents worked on a change,
  two lines, one from each.

## How to write it with jj

```bash
jj describe -m "fix(core): keep vault unlock error as Error::Secret

Why it changed.

Co-authored-by: Claude Code <noreply@anthropic.com>"
```

The same goes for `jj commit -m` and `jj new -m`. To check:

```bash
jj log -r @ --no-graph -T description | tail -3   # your line is in place, no duplicate
```

## Rules

- **Do not remove other people's trailers.** When rewriting a message (`jj describe -r …`,
  `jj squash`, `jj split`), keep every existing `Co-authored-by:` and
  add yours if it is not there yet.
- **Do not duplicate.** Before `describe`, look at the current message; if your
  line is already there, do not add a second one.
- **Auto-insertion is off on purpose.** If
  `jj config list templates.commit_trailers` prints anything, the template is
  enabled again and will insert someone else's name next to yours. Do not enable it, and
  tell the user if you find it.
- A commit is still made **only at the user's explicit request** (the
  `jj-workflow` skill).
- GitHub shows the `Co-authored-by` trailer as the co-author's avatar on the commit only
  if the email belongs to an account; `*.local` / `noreply` without an account do not, but
  the trailer stays in the commit text.
