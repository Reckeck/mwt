# Skill: TradeApp UI Design System

## Purpose
Enforce consistent UI across TradeApp. All agents modifying UI must follow these rules.

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

## Pre-Commit Checklist
- [ ] Spacing uses semantic tokens
- [ ] No hardcoded hex colors
- [ ] Typography follows hierarchy
- [ ] Forms: label→input `gap-1.5`, field→field `gap-4`
- [ ] Buttons use standard variants
- [ ] No nested modals
- [ ] Focus rings on all interactive elements
- [ ] Labels on all inputs
- [ ] `mise run check` passes

---

## For Agents: When Editing UI

1. **Read existing patterns first** — `grep -r "pattern" src/components/`
2. **Use semantic tokens** — never raw `p-3`, `gap-3`, `text-blue-500`
3. **Check existing patterns** in `src/components/ui/` first
4. **Run `mise run check`** before done (lint + typecheck + tests)
