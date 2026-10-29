---
name: subtask-pipeline
description: >
  The "planner -> executor -> review" pipeline: the strong model (Opus)
  writes the plan and the subtask brief, the weaker and cheaper one (Sonnet) carries it out
  in a subagent, and the strong one checks the result and accepts it or sends it back for
  rework. Use for any task from the plan that can be handed over
  whole: laying out a frame, an IPC command, a core test, a mock edit, screenshots, docs.
  Triggers: подзадача, subtask, подагент, subagent, Sonnet, исполнитель (executor),
  делегировать (delegate), бриф (brief), "отдай слабой модели" (hand it to the weaker model).
---

# Subtasks: Opus plans and reviews, Sonnet does

The roles do not change during the work:

| Role | Who | What it does | What it does not do |
|---|---|---|---|
| Planner and reviewer | the main session (Opus) | plan, breakdown, brief, review, gate, commit, report to the user | does not write itself what can be handed over by a brief |
| Executor | subagent `model: sonnet` (the general type, `general-purpose`) | edits code strictly per the brief, runs quick checks, reports | does not commit, does not change the plan, AGENTS.md or skills, does not widen the task |

In tools without a model choice for the subagent (OpenCode, Codex) the roles are the same:
the executor is the cheapest available model, the reviewer is the main session.

## When to hand over, when to do it yourself

- **Hand over:** the task fits a one-screen brief and is verified by a
  command or a screenshot: layout from a canvas frame, a new command following the
  `ipc-command` skill, a core test, a mock branch, locales, a docs section, a set of screenshots.
- **Do it yourself (Opus):** reading a frame and deciding where the code goes under FSD;
  contested design; debugging where the cause is not found yet (first find the cause
  per `debugging-and-error-recovery`, then hand over the fix); the vault schema and
  crypto (`vault-crypto`); commit, gate, report; an edit shorter than its brief.
- A subtask is split per `planning-and-task-breakdown`: one subtask is one
  verifiable result, usually 1-5 files. Each is a line in the task list
  with a number, and the brief refers to that number.

## The brief (what the executor receives)

The executor starts from zero and does not see the conversation. The brief is self-contained
(`context-engineering`): everything needed is inside it or given as a file path.

```
Task #<N>: <one line: a result, not an action>

Working copy: ~/apps/work-app-ui-redesign (jj; cwd is reset, always cd).
Read before editing: AGENTS.md, skills <list>, files <path:lines>.

What to do:
1. ...
How it should look / work: <canvas frame, path to the copy, texts verbatim>.

Acceptance criteria (all verifiable):
- [ ] ...
- [ ] quick checks are green: pnpm biome check --write src scripts;
      pnpm vue-tsc --noEmit; mise run check-ui; mise run check-fsd
      (+ cargo test -p trade-core <module> if core changed)

Forbidden: committing (jj describe/new/commit), git commit/add/stash, touching
files outside the list without a reason, changing AGENTS.md/the plan/skills, running
screenshots or mise run check in parallel with another process, changing the
user's data, widening the task.

Report (as the last message): changed files; what was done for each
criterion; check output (the tail); what failed and why; questions.
```

Do not rewrite the project's code rules in full in the brief (limits `.vue` 250 / `.ts` 300 lines, `meta` on a mutation,
ru+en locales, FSD, shared components): name the skills and `AGENTS.md`, and in the criteria list what is
easy to violate in this particular task.

## Launch

- `Agent` with `model: "sonnet"` and `description` "#<N> <short>". The subagent
  works in the same jj working copy: **one executor at a time** if
  the subtasks touch shared files or start Vite (screenshots, check-layout and the
  gate share a port). In parallel only subtasks that are independent by file and do not
  start a server.
- Not `isolation: "worktree"`: AGENTS.md forbids git worktree in this repo.
- While the executor works, the planner does not edit the same files; it can
  prepare the next subtask's brief or read a frame.

## Review (what Opus does after the report)

Do not take the report on faith; check against the facts (`code-review-and-quality`):

1. `jj diff --stat` and `jj diff <files>`: only what is in the brief; no
   stray edits, debug code, `// DEBUG`, commented-out code.
2. Re-run the quick checks yourself (do not take the output from the report).
3. Layout: screenshots `mise run shots -- <set> --only <name>` at 1280 and 390,
   look at the "canvas | app" pairs through Read.
4. Logic: by the code: edge cases, errors, domain arithmetic only
   in `crates/core`, texts verbatim from the frame, both locales.
5. Decision:
   - **accept**: mark the task done;
   - **send back**: `SendMessage` to the same executor with a list of concrete
     remarks (file:line -> what is wrong -> what is expected); no more than two
     returns, then finish it yourself and record why;
   - **roll back**: `jj restore <files>` if the result is unfit, and
     rewrite the brief (usually the cause is the brief).
6. Deviations from the canvas go to `docs/ui-redesign-vs-canvas.md` (written by whoever
   made the edit, checked by Opus).

The gate (`cargo fmt` + `mise run check`), the commit (`jj-workflow`,
`commit-coauthor`) and the report to the user belong to the planner only, after
all subtasks of the stage have been reviewed. In the report to the user: which subtasks
the executor did and how many returns there were.
