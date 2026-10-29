---
name: workflow
description: >
  How an agent runs any session in this repository: load it as the first action
  in every new chat, before any other work. Analysis, search and changes are handed to
  subagents; the main session plans, writes the brief, verifies and reports.
  Triggers: session start, начало сессии, new chat, новый чат, workflow, "how to work" ("как работать"),
  subagent, подагент, agents, analysis, анализ, search, поиск, changes, изменения, delegate, делегировать.
---

# Workflow: how a session runs

## When to load

- As the first action in every new chat or session, before reading code.
- When the user writes "use workflow" (Russian: «используй workflow»).

## Subagents: the default

Three kinds of work are done by a subagent (Claude Code: the `Agent` tool;
OpenCode and Codex: their task/subagent):

- **analysis**: reading code, causes of bugs, reviews;
- **search**: grep sweeps, "where is this", logs;
- **changes**: edits to code, documents, `.figcraft` mockups.

The main session: plan, brief, verification of the result (diff, gate, screenshots),
the report to the user, a commit on request.

Exceptions where delegating is not needed: a one-line answer from already open
context, a one-line edit, a question to the user.

Delegation rules:

- The brief is self-contained: paths, ids, conventions, the definition of done and what is
  forbidden (commits, worktrees, going beyond the task). How to write a brief and launch
  an executor: the `subtask-pipeline` and `context-engineering` skills.
- The executor is the weaker model (Sonnet), the review is done by the strong one; the roles are as in
  the "Subtasks" section of `AGENTS.md`.
- One executor at a time per file or working copy: parallel edits of
  one file, Vite and figcraft-cli race each other. Independent
  searches can run in parallel.
- A subagent does not commit and does not widen the task.
- The main session re-checks the subagent's report (diff, checks) instead of
  retelling it on faith.

## Working copies

Only `../work-app-<slug>` via `jjw-start <slug>`; give the user the path
in the very first message about the task. If the main copy is dirty:

```bash
jj workspace add ../work-app-<slug> --name work-app-<slug> -r development
jj bookmark create <slug>
```

Never `EnterWorktree`, `isolation: "worktree"`, `.claude/worktrees/`,
`git worktree add`. Details: "Working copies" in `AGENTS.md`.

## Commits and the gate

The `jj-workflow` and `commit-coauthor` skills. Commit only at the user's explicit
request; the gate (`cargo fmt`, `mise run check`, `mise run check-platforms`)
is run by the main session itself before the commit.

## Questions to the user

Through the question tool (`AskUserQuestion`), with a recommended option.
A large task: first a spec and a plan (the plan lives outside the repository, see "Subtasks" in
`AGENTS.md`), then the implementation.

## Skills

Before editing, load the relevant skill from the "Agent skills" table in
`AGENTS.md`: `ui-design-system`, `onboarding`, `ipc-command`, `vault-crypto`,
`frontend-feature`, `figcraft` and others. In the subagent's brief, list which skills
it should load.
