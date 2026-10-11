# TradeApp UI Design System

> Single source of truth for spacing, typography, color, components, and accessibility rules.
> All UI work must follow these rules. Agents must reference this file.

---

## 1. Spacing System

**Base unit: 4px** (Tailwind `1` = 4px). All spacing = multiples of 4px.

| Token | Value | Tailwind | Use for |
|-------|-------|----------|---------|
| `xs` | 0.5 (2px) | `0.5` | tight gaps, icon padding |
| `sm` | 1 (4px) | `1` | button gaps, input icon gap |
| `md` | 1.5 (6px) | `1.5` | label→input, form gap |
| `lg` | 2 (8px) | `2` | button gaps, small margins |
| `xl` | 3 (12px) | `3` | field→field |
| `2xl` | 4 (16px) | `4` | card padding, page padding |
| `3xl` | 6 (24px) | `6` | section gap, card lg |
| `4xl` | 8 (32px) | `8` | page padding lg |

### Semantic Aliases (use these in components)

```ts
// Form
formGap: 1.5      // label → input
formFieldGap: 4   // field → field
formSectionGap: 6 // section → section

// Layout
cardPadding: 4      // p-4
cardPaddingLg: 6    // p-6
sectionGap: 6       // gap-6 between sections
pagePadding: 4      // p-4
pagePaddingLg: 6    // p-6

// Components
btnGap: 2       // icon + text in buttons
inputGap: 2     // icon + text in inputs
iconSize: 4     // 16px icons

// Grid
gridGapMd: 4   // gap-4 standard grid
gridGapLg: 6   // gap-6 loose grid
```

### Tailwind Class Helpers
```ts
SpacingClasses = {
  // Gaps
  gapXs: 'gap-0.5', gapSm: 'gap-1', gapMd: 'gap-1.5', gapLg: 'gap-2',
  gapXl: 'gap-3', gap2xl: 'gap-4', gap3xl: 'gap-6', gap4xl: 'gap-8',

  // Padding
  pXs: 'p-0.5', pSm: 'p-1', pMd: 'p-1.5', pLg: 'p-2',
  pXl: 'p-3', p2xl: 'p-4', p3xl: 'p-6', p4xl: 'p-8',

  // Semantic combos
  card: 'p-4', cardLg: 'p-6', section: 'gap-6',
  formField: 'gap-4', formGap: 'gap-1.5',
  gridMd: 'gap-4', gridLg: 'gap-6', page: 'p-4 md:p-6',
}
```

---

## 2. Typography

| Role | Size | Weight | Tailwind |
|------|------|--------|----------|
| Meta / caption | 12px | 400 | `text-xs` |
| Body / UI | 14px | 400 | `text-sm` |
| Body emphasized | 14px | 500 | `text-sm font-medium` |
| Heading 3 | 16px | 600 | `text-base font-semibold` |
| Heading 2 | 18px | 600 | `text-lg font-semibold` |
| Heading 1 / Page title | 20px | 600 | `text-xl font-semibold` |

- **Body**: `text-sm` (14px) default
- **Reading text**: `text-base` (16px)
- **Weights**: `font-normal` (400) body, `font-medium` (500) emphasis, `font-semibold` (600) headings

---

## 3. Color & Contrast

**Use semantic tokens only** — no hardcoded hex.

| Purpose | Token |
|---------|-------|
| Primary text | `text-foreground` |
| Muted text | `text-muted-foreground` |
| Background | `bg-background` |
| Card surface | `bg-card` |
| Muted surface | `bg-muted` |
| Borders | `border-border` |
| Primary action | `bg-primary text-primary-foreground` |
| Destructive | `bg-destructive text-destructive-foreground` |
| Focus ring | `focus-visible:ring-2 focus-visible:ring-ring` |

- **Min contrast**: 4.5:1 text, 3:1 UI elements
- **No hardcoded hex** — use CSS variables / Tailwind semantic tokens

---

## 4. Components

### Border Radius
| Use | Radius | Tailwind |
|-----|--------|----------|
| Default UI | 6px | `rounded-md` |
| Cards / Panels | 8px | `rounded-lg` |
| Pills / Avatars / Badges | full | `rounded-full` |

### Shadows
| Elevation | Use | Tailwind |
|-----------|-----|----------|
| Subtle | Cards at rest | `shadow-sm` |
| Elevated | Hovered cards, dropdowns | `shadow-md` |
| Modal / Overlay | Dialogs, popovers | `shadow-lg` |

### Transitions
- Color: `transition-colors duration-150`
- Transform: `transition-transform duration-150`
- Combined: `transition-all duration-150` (avoid unless needed)

### Forms
- Label above input, `gap-1.5` label→input, `gap-4` field→field
- Inline validation: `text-xs text-destructive` below field
- Disabled: `opacity-50 cursor-not-allowed`
- Input: `h-9 rounded-md border border-input bg-transparent px-3 text-sm`

### Buttons
```tsx
// Variants
default: 'bg-primary text-primary-foreground hover:bg-primary/90'
outline: 'border border-input bg-background hover:bg-accent'
ghost: 'hover:bg-accent hover:text-accent-foreground'
destructive: 'bg-destructive text-destructive-foreground'
link: 'text-primary underline-offset-4 hover:underline'

// Sizes
default: 'h-10 px-4 py-2'
sm: 'h-9 rounded-md px-3'
lg: 'h-11 rounded-md px-8'
icon: 'h-10 w-10'
```

### Tables & Lists
- Row padding: `py-2 px-3` (compact) / `py-3 px-4` (comfortable)
- Hover: `hover:bg-accent` for clickable
- Numbers: `tabular-nums text-right`, text `text-left`

### Modals / Dialogs
- **No nested modals** — use tabs/steps inside single dialog
- Max width: `max-w-md` (forms), `max-w-lg` (content), `max-w-4xl` (full)
- Close: ESC, backdrop click, × button, submit/cancel
- Always pass a `description` — Reka warns when `DialogContent` has none, and
  screen readers get nothing to announce for a settings dialog otherwise

### Combobox (select with search)
`src/entities/currency/ui/currency-select/CurrencySelect.vue` — Reka `Combobox*` primitives, used
for every currency picker (base currency, favourites).

- **Shape**: trigger looks like a `<Select>`; the panel has a search input on top
  and the option list below.
- **Anchoring**: `ComboboxAnchor` + `ComboboxTrigger`, both `as-child`, so the
  DOM stays one `<button>`. Anchor registers the popper reference, Trigger
  toggles the panel — neither alone is enough.
- **`position="popper"` is mandatory.** `ComboboxContent` has no default for it,
  and without it the panel renders inline at the bottom of the page instead of
  under the trigger.
- **Width var** is `--reka-combobox-trigger-width` (no `content-` in the name).
- **Override two Reka defaults** on the trigger: it hardcodes an English
  `aria-label="Show popup"` and `tabindex="-1"`. The second makes the control
  unreachable by Tab while the panel is closed — the only tab stop there.
- The panel input is a **pure filter**: pass `:display-value="() => ''"`, else it
  pre-fills with the current value and typing appends to it.
- Long lists are capped (300) with a visible "showing N of M" hint, never
  silently truncated.

---

## 5. Layout & Responsive

- **Mobile-first**: base styles mobile, `md:`/`lg:`/`xl:` for desktop
- **Grid**: `grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4`
- **Container**: `max-w-7xl mx-auto px-4 md:px-6`
- **Page wrapper**: `min-h-0 flex-1 overflow-y-auto p-4 md:p-6 @container`

---

## 6. Accessibility (WCAG 2.1 AA)

- **Focus rings**: `focus-visible:ring-2 focus-visible:ring-ring`
- **Labels**: every input has `<Label>` or `aria-label`
- **Keyboard**: all interactive elements reachable, logical tab order
- **ARIA**: `aria-expanded`, `aria-controls` for expandable sections
- **Min contrast**: 4.5:1 text, 3:1 UI elements
- **Reduced motion**: respect `prefers-reduced-motion`

---

## 7. Migration Checklist (when updating a view/component)

- [ ] Spacing uses semantic tokens / `SpacingClasses` helpers
- [ ] No hardcoded `p-2`, `gap-3`, etc. — use semantic aliases
- [ ] No hardcoded hex colors — use semantic tokens
- [ ] Typography follows hierarchy table
- [ ] Border radius follows table
- [ ] Forms use `gap-1.5` label→input, `gap-4` field→field
- [ ] Buttons use standard variants
- [ ] No nested modals
- [ ] Focus rings present
- [ ] Labels on all inputs
- [ ] Keyboard accessible

---

## 8. For Agents (Implementation Rules)

When editing UI code:

1. **Check existing patterns first** — grep for similar components
2. **Use semantic spacing tokens** — never raw `p-3`, `gap-3`, etc.
3. **No hardcoded colors** — use semantic tokens only
4. **Typography** — follow hierarchy table exactly
5. **Forms** — label→input `gap-1.5`, field→field `gap-4`
- **Buttons** — use standard variants from `shared/ui/button`
6. **No nested modals** — refactor to tabs/steps
7. **Run `mise run check`** before committing (lint + typecheck + tests)

## 9. Interaction patterns (wallets redesign)

### Right-click menus
- Every block an action applies to has a context menu: `shared/ui/context-menu`
  (`ContextMenu` + `UiAction[]`). The same list feeds the «⋯» button
  (`shared/ui/action-menu`), so the two never drift apart.
- Order: open / navigate → main actions → copy → configure → dangerous.
  Dangerous items are `destructive: true` (red text **and** red icon, red-tinted
  highlight — `MENU_ITEM_DESTRUCTIVE_CLASS`), always last, after a separator.
  Ordinary icons stay muted.
- An unavailable action is shown disabled with the reason (`disabled: 'string'`);
  only actions that do not apply to the object at all are `hidden`.
- The WebView's native menu is suppressed globally except in inputs, textareas
  and CodeMirror (copy/paste stays).

### Dashboard grid
- `grid-layout-plus`, 12 columns, 40 px rows, vertical compaction: no gaps.
- `shared/ui/widget-frame`: the header is the drag handle; «Hide header» pins the
  widget (static, not resizable). Widget content is a `@container`, so cards adapt
  to their cell through container queries.
- One registry: `widgets/dashboard-registry` (id, title, description, component,
  sizes). The floating «+» opens `widgets/dashboard-catalog` with live,
  scaled-down previews.

### Settings window
- `widgets/settings-shell`: a dialog inset 120 px (16 px on narrow screens), tabs
  on the left; the active tab lists its sections, hover or click scrolls to one,
  scrolling highlights it. Deep link: `?settings=<tab>&section=<id>`.

### Guided tours
- `shared/lib/tour` on `driver.js`: steps point at `[data-tour="…"]`, missing
  elements are skipped. A tour auto-starts once per screen; «?» in the header
  replays it. Finished tours live in `AppSettings.completed_tours`.
