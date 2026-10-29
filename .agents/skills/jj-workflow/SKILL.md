---
name: jj-workflow
description: >
  Rules for checks and commits in this repository via Jujutsu (jj, colocated
  with git). Use before any commit, and for questions about history, undo,
  branches, moving changes between features, reverting edits, splitting
  changes, renaming or squashing commits, the Co-authored-by trailer and
  marking agent authorship, rescuing "forgotten" code. Russian triggers:
  коммит, закоммить, откат, ветка, «проверки перед коммитом», «прогони тесты»,
  соавтор, автор, авторство, «кто написал». Triggers: commit, jj, jujutsu,
  describe, new, squash, split, rebase, bookmark, branch, undo, revert, stash,
  diff, status, co-author, Co-authored-by.
---

# Checks and commits via Jujutsu

The repository is under **jj** (jj 0.45+, colocated with git: `.git` is a real
git repository, `.jj` is jj metadata). Manage history with **jj commands**, not
git commands.

## The main difference from git

jj has no index and no "unstaged/staged". Changes in files are **automatically**
snapshotted into the change `@` at the next jj command. Therefore:

- **`git add` is not needed and does not exist.** A file you created is already
  in `@`.
- **`jj status` shows the full picture**, not "what is staged". Empty output
  means "the working copy matches the last change".
- **`jj commit` = `jj describe` + `jj new`.** These are two operations, not one.
- **Every `jj new` leaves the previous change in history.** There is no need to
  "rewrite" the message after the fact — start the next change.
- **Branches are bookmarks**; they point to a change and do not store history:
  `jj bookmark create feature-x -r @-`, `jj bookmark move main`, `jj git push --bookmark main`.

## Gate before committing (mandatory)

```bash
cargo fmt
mise run check      # cargo xtask check: parallel, fail-fast, logs in .cache/xtask/
```

`mise run check` is the **full** gate: Biome (lint + format), strict frontend
typecheck, rule checks (`check-*.mjs`, layout/links `--quick`), `cargo fmt --check`,
clippy `-D warnings`, all workspace tests, freshness of `ipc.gen.d.ts` and
compilation for all platforms — in parallel, the first error stops the rest.
Gate green → you may commit. Not green → commit is forbidden: fix the cause
first.

A local slice, if the change is obviously narrow (faster, but does not replace
the final `check`):

| What you touched | What to run |
|---|---|
| `src/**` | `mise run lint && mise run typecheck` |
| `crates/**` | `mise run test` |
| `ipc_contract.rs`, `model/**`, any DTO | `mise run test && mise run gen-types` |
| before a commit / refactoring / release | `mise run check` |

**If you changed DTOs or domain structs that go into IPC:** after
`mise run gen-types`, `src/shared/api/ipc.gen.d.ts` must change. If the file did
not change, the type is not covered by ts-rs (`#[derive(TS)]` is missing), and
the frontend will not get the type. Check this with a diff, not by eye.

Never "fix" a failing test by deleting the test, weakening the assert, or
`#[ignore]`. The cause of the failure is either your bug or a stale expectation.

## Commit order

```bash
jj status                     # 1. what will actually go into the change
jj diff                       # 2. read the whole diff — there may be someone else's changes
cargo fmt && mise run check   # 3. gate green
jj describe -m "fix: ..."     # 4. describe the change
jj new                        # 5. next change
```

Step 2 is not a formality: the working copy accumulates everything you edited
without committing, including debug `console.log`, commented-out code and edits
for other tasks. Before describing a change, make sure it contains only your
change.

## Message format

Conventional commits, **in English**, subject ≤ 72 characters, no trailing
period.

```
fix: keep vault unlock error as Error::Secret

Argon2id derives a different key on every attempt, so a wrong password
surfaced as Error::Crypto, which the UI localized as a generic failure
instead of prompting for a retry. Map the tag mismatch to Error::Secret
so the unlock form can show auth.wrongPassword.
```

- Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`.
- The subject is what changed for the user, not a list of files.
- The body is **mandatory** if the change is not obvious: *what* → *why*.
  Do not retell the diff; explain the motivation and the alternatives you
  rejected.
- One logical change — one change. Do not mix `feat:` and `refactor:`, do not
  drag incidental formatting into a commit.
- Breaking changes: `feat(scope)!: …` + a `BREAKING CHANGE:` paragraph in the body.
- A scope (`feat(vault):`, `fix(pricing):`) is optional — use it when it adds
  information.

## Co-author: the agent names itself

Every agent commit carries a `Co-authored-by:` trailer with the **name of its
own tool** (OpenCode, Claude Code, Codex, …). jj does **not** add it —
`templates.commit_trailers` is turned off on purpose — the agent appends the
line itself, as the last paragraph of the message. The format, the name table,
the duplicate check and the rule "do not touch other people's trailers" are in
the `commit-coauthor` skill; load it before `jj describe`.

## Commit — only on explicit request

The user must **directly ask** to commit. The mere fact that "the task is done"
is not a reason: finished the work → stop and report. Edits in the working copy
(`jj status` non-empty) are a normal state; committing them without a request is
forbidden.

## When something went wrong

| Situation | What to do |
|---|---|
| Forgot `jj new`, described the wrong change | `jj log` → find the change → `jj describe -r <rev> -m "…"` |
| The change turned out non-atomic | `jj split -r @` interactively or `jj squash` afterwards |
| Changes landed in "someone else's" change | `jj restore -c <rev> -s <paths>` (jj ≥ 0.30), or `jj diff -r <rev> > p.patch` + `jj restore` |
| Need to move edits to another change | `jj git push --change <rev> --target <dest>` |
| Want to "undo as if I never did it" | `jj op log` → `jj undo` / `jj restore -r @-` |
| Need to rewrite something not yet pushed | `jj rebase -r @ -d @-` |
| Mistakenly pushed to main | **do not** rewrite: `jj bookmark move main <old-rev>` + warn the user |

Undo in jj is **complete and safe**: the working copy and history roll back
entirely (`jj undo`, `jj op log`). Do not invent manual "roll-backs" through git
checkout.

## Boundaries that must not be crossed

- **`git commit`, `git add`, `git stash`, `git checkout <file>`, `git reset --hard`** —
  do not use. In a colocated repository they desynchronize the jj snapshot from
  the git state. The only acceptable use of git is read-only
  (`git diff`, `git log`) for cross-checking, and even then `jj log`/`jj diff`
  can do more.
- **Do not rewrite a change already pushed to the server.** Public history is
  immutable by convention, not technically. Revisions change on every
  `jj new` — identify a change by its **change id** (the change id column of
  `jj log`), not by commit id.
- **Do not commit** `data/`, `*.wallet`, `*.wallet.bak`, `.rust-target/`,
  `node_modules/`, `dist/`, `.env*`. All of this is already in `.gitignore`, and
  jj respects it — but if a file got into a change (for example, forcibly
  created before jj was initialized), check `jj status` before `jj describe`.
- **Do not run `jj git push` without the user's permission.**
- **`jj new` on a dirty copy is normal**, but do not leave a change described as
  "wip" for long: it will land in history and later require `jj rebase`.

## Check before reporting

```bash
jj status    # what remains uncommitted (tell the user honestly)
jj log -r '::@' --no-graph -T 'description.first_line() ++ "\n"'   # messages of my changes
```

If after your work `jj status` shows uncommitted changes, that is normal, but
**name them to the user explicitly** instead of silently leaving them.
