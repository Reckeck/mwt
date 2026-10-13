---
name: ui-design-system
description: Enforces Multi Wallet Tracker UI consistency — spacing tokens, semantic colors, typography, form layout, dialogs and feedback. Use when adding or changing any UI in src/** — new screen, form, table, dialog, toast, loading or error state, button, card. Triggers on .vue, view, component, form, button, card, dialog, toast, sonner, spinner, skeleton, badge, notification, loading state, empty state.
---

# Skill: Multi Wallet Tracker UI Design System

Consistent UI across Multi Wallet Tracker. Load this **together with** `shadcn-vue` (in
`.agents/skills/shadcn-vue/`) whenever you touch a component: that skill knows
how to add and compose shadcn primitives, this one knows how Multi Wallet Tracker looks.

Primitives live in `src/shared/ui/<name>/` — one folder per component with
an `index.ts` (AGENTS.md rule 11). shadcn-vue's CLI writes there directly
(`components.json` aliases); answer "no" to overwriting the hand-made
button/input, and drop any `@lucide/vue` it adds — the project uses
`lucide-vue-next`.

---

## Quick Reference

### Spacing Tokens (use these, never raw values)
```ts
formGap: 1.5      // label → input
formFieldGap: 4   // field → field
formSectionGap: 6 // section → section
cardPadding: 3    // p-3 (Card size="md": p-3 sm:p-4)
cardPaddingLg: 4  // p-4
sectionGap: 4     // gap-4
btnGap: 2
inputGap: 2
iconSize: 4       // 16px
```

### Typography Scale
| Role | Size | Weight |
|------|------|--------|
| Meta / caption | 12px | 400 |
| Body / UI | 14px | 400 |
| Body emphasized | 14px | 500 |
| H3 | 16px | 600 |
| H2 | 18px | 600 |
| H1 | 20px | 600 |

### Semantic Colors Only
- `text-foreground`, `text-muted-foreground`
- `bg-background`, `bg-card`, `bg-muted`
- `border-border`, `bg-primary`, `bg-destructive`
- **No hardcoded hex**

### Border Radius
- `rounded-md` (6px) default UI
- `rounded-lg` (8px) cards
- `rounded-full` pills/avatars

### Forms
- Label→input: `gap-1.5`
- Field→field: `gap-4`
- **Required fields carry a red `*`** — `<FormField required>` / `<Label required>`;
  `AssetInput` and `CurrencyPicker` take `required` and `error` themselves.
- **The save button stays enabled.** Only a save in flight (`isPending`) or a
  backend refusal whose reason is shown next to it may disable it.
- **Errors appear on save, under the field.** Pressing save with a missing or
  wrong field does not save: each such field gets its message
  (`text-xs text-destructive`, `role="alert"`) and a red border, and the focus
  goes to the first one. After that the messages follow the input.
- One flow for all of it — `useFormValidation` from `@/shared/lib/form`:

```vue
<script setup lang="ts">
const form = useFormValidation(
  () => ({
    name: required(name.value),
    limit: positiveAmount(limit.value),        // blank / not a number / ≤ 0
    currency: required(currency.value, 'pick'), // «Выберите значение»
  }),
  { resetOn: open },                            // a reopened dialog starts clean
)
</script>

<template>
  <form id="budget-form" @submit.prevent="form.submit(save.mutate, $event)">
    <FormField for="budget-limit" :label="t('budget.limit')" required :error="form.error('limit')">
      <AmountInput id="budget-limit" v-model="limit" />
    </FormField>
  </form>
  <Button form="budget-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
</template>
```

- A button outside a `<form>`: `@click="form.submit(save.mutate, $event)"`.
- A one-line «add» row with no labels: wrap each control in
  `<span class="contents" :data-invalid="form.invalidAttr('name')">` and put
  `form.firstMessage()` in one line under the row; call `form.reset()` after
  a successful add, or the cleared field shows its error at once.
- Validators (`validators.ts`): `required`, `decimal`, `positiveAmount`,
  `nonNegativeAmount`, `integerInRange`, `minLength`, `firstError`. Texts live
  under `validation.*` and say how to fix it.
- Mirror backend rules the form can know (a top-up or payment needs a
  category, V5): a bare «Invalid input» toast after saving is a form bug.
- Gate: `mise run check-forms` (part of `mise run check`).

### Choosing an account, an asset, a currency (rule 14)
Never a hand-made `<Select>` + `<option v-for>` or an `<Input>` for a code —
the ready component, everywhere (forms, filters, settings, workflow fields):

| What | Component | Notes |
|---|---|---|
| wallet / sub-account | `StoreNodeSelect` (`entities/store`) | shows the path «Wallet › Sub»; `flat` for a filtered list, `placeholder` (required pick), `emptyLabel` («none»), `invalid` |
| wallet, contact or free text in an operation | `AccountRefPicker` (`features/transaction-create`) | |
| any asset (search) | `AssetInput` (`entities/asset`) | `required`, `error` |
| one of known assets (held) | `AssetSelect` (`entities/asset`) | model is the `AssetRef`; `placeholder`, `invalid` |
| currency | `CurrencyPicker` (`entities/currency`) | favourites first; `extra`, `kinds`, `allowEmpty`, `required`, `error` |

A case the component cannot do → add a prop to it, don't write a list next
to it. `check-forms` flags `<option v-for>` over wallets/assets/currency codes
and `<Input v-model="…currency…">`.

### Buttons
Use `shared/ui/button` variants: `default`, `outline`, `ghost`, `destructive`, `link`

### Modals
- **One dialog per action.** Two buttons that do different things open two
  dialogs (see `TransactionCreateDialog` / `PlanCalculatorDialog`); never one
  dialog that switches content with tabs. Hand-offs close one, open the next.
- **No chrome inside a dialog:** no Card, no second title. The dialog has the
  title; content is bare; the primary action sits in `#footer` (right-aligned,
  `form="…"` for submit). `Dialog` brings its own close button.
- Content with `@sm:`/`@lg:` container queries needs a `@container` wrapper
  inside the dialog — the portal is outside the page container.
- **NO NESTED MODALS.** Max width: `sm` (forms), `lg` (content), `xl` (full).

### Destructive actions
- Confirm in place with `shared/ui/confirm-button` (click → "Delete?" → click),
  not with a modal. Disable the action with the reason when the backend
  would refuse it (e.g. deleting a wallet that has history).

### Charts (vue-echarts)
- Put the height on a wrapper: `<div class="h-80"><Chart … /></div>`.
  vue-echarts 8 forces `height: 100%` on its element from an unlayered
  stylesheet that beats Tailwind's utilities, so `h-*` on `<Chart>` itself
  yields a 0px canvas.

---

## Feedback: everything that loads or sends a request must notify

**Правило: любой экран, который что-то грузит или отправляет, обязан сообщать
об этом пользователю — если задача не говорит об обратном.**

A request that finishes silently reads as a dead button. A request that fails
silently is worse: the user changes a setting, sees nothing, and assumes it
saved.

Applies to **every** `call<T>()` / `useMutation` / `onEvent` call:

| Situation | What to show |
|---|---|
| Save succeeded | `toast.success(t('...'))` |
| Save failed | `toast.error(...)` — **never** only `console.error` |
| Long operation running | pending state on the control (`disabled` + spinner) |
| Background refresh finished | toast only if it changed something user-facing |
| Destructive action | confirm, then toast on the outcome |

### How

Toasts are `vue-sonner`. `<Toaster />` is mounted once in `AppShell`, so a view
only needs the import:

```ts
import { toast } from 'vue-sonner'
import { t } from '@/i18n'          // or useI18n() inside setup

toast.success(t('wallet.saved'))
toast.error(errorText(parseError(err)))
```

Rules for the messages themselves:

- **Through `t()`.** No hardcoded strings — the UI is bilingual.
- **`toast.error` on every failure path.** An empty `catch {}` is a bug, not a
  style choice.
- **Never both a toast and an inline `<p>` for the same event.** Pick the toast
  for the transient outcome; keep inline text only for state that must stay
  visible while the user looks at the field (validation, sync status).
- **No toast per keystroke.** Debounce, and skip repeats of the same message.

### What is *not* a notification

Persistent, glanceable state is not a toast and must not become one:

- empty-state copy, sync badges, persistent warnings, validation hints.

A toast is transient. If the information must survive the toast's lifetime, it
belongs on the screen.

---

## Pre-Commit Checklist
- [ ] Spacing uses semantic tokens
- [ ] No hardcoded hex colors
- [ ] Typography follows hierarchy
- [ ] Forms: label→input `gap-1.5`, field→field `gap-4`
- [ ] Forms: required fields have `*`, save stays enabled, errors under fields via `useFormValidation`
- [ ] Accounts, assets, currencies are picked with `StoreNodeSelect` / `AccountRefPicker` / `AssetInput` / `AssetSelect` / `CurrencyPicker` (rule 14)
- [ ] Buttons use standard variants
- [ ] No nested modals
- [ ] Focus rings on all interactive elements
- [ ] Labels on all inputs
- [ ] **Every request reports success and failure** (toast), or the task
      explicitly says otherwise
- [ ] No `console.error` standing in for user-visible feedback
- [ ] All new UI strings exist in **both** `ru.json` and `en.json`
- [ ] `mise run check` passes

---

## For Agents: When Editing UI

1. **Read existing patterns first** — `grep -r "pattern" src/components/`
2. **Check `src/shared/ui/`** before adding a primitive; prefer the shadcn
   CLI (`pnpm dlx shadcn-vue@latest add <component>`)
3. **Use semantic tokens** — never raw `text-blue-500`
4. **Wire the feedback** — see the rule above; it is not optional
5. **Run `mise run check`** before done (lint + typecheck + tests)