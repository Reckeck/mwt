---
name: onboarding
description: >
  Teaching the user together with every trade-app feature (AGENTS.md, rule 12):
  driver.js tours via `data-tour`, in-place hints, empty states, errors that
  explain, "What's new" and the learning center. Use for any user-visible
  change: new screen, dialog, form, field, action, behavior change. Russian
  triggers: тур, обучение, «Что нового», подсказка, пустое состояние,
  «добавь функцию», «доработай». Triggers: tour, onboarding, data-tour, TOURS,
  whats-new, empty state, check-onboarding.
---

# Onboarding is part of the feature

A feature without onboarding is not done. The user does not read documentation:
they learn about a capability from a tour, a hint under a field, or the
"What's new" window.

## What counts as a feature

These need onboarding: a new screen or tab, a new dialog or form, a new action
(button, menu item), a new non-obvious field, changed behavior (something that
used to work differently).

These do not (put a line `Onboarding: n/a — <reason>` in the commit):
refactoring, performance, a fix without a behavior change, tests,
documentation.

## Checklist

1. **Tour.** A new screen (route) gets a tour in `src/shared/lib/tour/tours.ts`
   (`TOURS[<route name>]`), 2-5 steps. A significant new element on an old
   screen gets a step in its tour. A route without a tour is allowed only with
   a reason in `TOURLESS_ROUTES`.
2. **In-place hint.** A non-obvious field gets `hint` on `FormField` (or
   `title`). An empty list gets `EmptyState`/`SectionState`: what will be here
   and where to start, with a first-action button. The look is in the
   `ui-design-system` skill; here it is about what to say.
3. **Errors explain how to fix.** The save button is not blocked by field
   validation (rule 13): pressing it shows what is missing under each field; a
   long form also sums this up in a line "To save, you still need…"
   (`TransactionForm`). A button blocked by a backend refusal states the
   reason next to it.
4. **"What's new".** A noticeable behavior change gets an entry in
   `src/shared/config/whats-new.ts` (a new record for the release or an item in
   the current one), with `route` if there is a tour that shows it.
5. **Texts in both locales.**
6. **`mise run check-onboarding`** is green (part of `mise run check`).

## How to add a tour step

```vue
<Button data-tour="wallet-adjust" @click="…">…</Button>
```

```ts
// src/shared/lib/tour/tours.ts
wallet: [
  { target: 'wallet-adjust', key: 'wallet.adjust', side: 'bottom' },
  …
],
```

Texts are `tour.<key>.title` and `tour.<key>.body`; the tour name in the
learning center is `tourName.<route>`. A step whose element is not on screen
right now is skipped automatically: one tour serves wallets of all kinds.
Elements inside dialogs are not reachable by route tours; they are explained by
the dialog description (`description` on `Dialog`) and field hints.

A tour starts by itself on the first visit to a screen, by the "?" in the
header, and from the learning center (Settings → General → Tours) via
`?tour=<route>`; leaving the page closes it without counting it as done.

## How to write the texts

General copy rules (tone, vocabulary, "…", sentence case) are in
`ui-design-system/references/copy.md`. For onboarding, additionally:

- Short: a title of 1-3 words, a body of at most two sentences.
- Start from the action and the benefit: "Enter the actual balance — the
  difference will be recorded as an adjustment", not "This component lets you…".
- No internal terms (`store`, `binding`, `flow_id`, "sub-account of type
  market"). Say "wallet", "account", "chain of operations".
- Bad: "Here you can configure the parameters". Good: "The purpose of money
  decides whether spending counts toward the daily budget".

## Lessons for complex features

If a feature cannot be understood in 3-5 steps (node-based automation, account
reconciliation), it needs a lesson: a task in a sandbox
(`UnlockedVault::sandbox`) with a check of the result. Lessons live in the
learning center next to the tours.

## Check

```bash
mise run check-onboarding   # screen without a tour, step without an element, missing text — error
```

The script is static (`tests/checks/onboarding.check.ts`): it reads the routes,
`TOURS`, `WHATS_NEW`, `data-tour` in `.vue` files and both locales. A
`data-tour` that is in no tour is only a warning.
