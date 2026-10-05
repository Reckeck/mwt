---
name: ui-design-system
description: Enforces TradeApp UI consistency — spacing tokens, semantic colors, typography, form layout, dialogs and feedback. Use when adding or changing any UI in src/** — new screen, form, table, dialog, toast, loading or error state, button, card. Triggers on .vue, view, component, form, button, card, dialog, toast, sonner, spinner, skeleton, badge, notification, loading state, empty state.
---

# Skill: TradeApp UI Design System

Consistent UI across TradeApp. Load this **together with** `shadcn-vue` (in
`.agents/skills/shadcn-vue/`) whenever you touch a component: that skill knows
how to add and compose shadcn primitives, this one knows how TradeApp looks.

Component source lives flat in `src/components/ui/<name>.vue` (lowercase, no
per-component directories) — follow that when adding a primitive.

---

## Quick Reference

### Spacing Tokens (use these, never raw values)
```ts
formGap: 1.5      // label → input
formFieldGap: 4   // field → field
formSectionGap: 6 // section → section
cardPadding: 4    // p-4
cardPaddingLg: 6  // p-6
sectionGap: 6     // gap-6
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
- Validation: `text-xs text-destructive` below field

### Buttons
Use `components/ui/button.vue` variants: `default`, `outline`, `ghost`, `destructive`, `link`

### Modals
- **NO NESTED MODALS** — use tabs/steps in single dialog
- Max width: `max-w-md` (forms), `max-w-lg` (content), `max-w-4xl` (full)

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
2. **Check `src/components/ui/`** before adding a primitive; prefer the shadcn
   CLI (`pnpm dlx shadcn-vue@latest add <component>`)
3. **Use semantic tokens** — never raw `text-blue-500`
4. **Wire the feedback** — see the rule above; it is not optional
5. **Run `mise run check`** before done (lint + typecheck + tests)