# Tokens: color, text, spacing, radii, theme

The source of truth is `src/app/styles.css` (`@theme` + `:root` + `.dark`) and
the component variants in `src/shared/ui/*`. Here is how to use them; numbers
that live in component code (button height, focus ring) are not retold: take
the component variant.

## Color

| Meaning | Class | Not like this |
|---|---|---|
| main text / background | `text-foreground`, `bg-background`, `bg-card` | `text-black`, `bg-white` |
| secondary text | `text-muted-foreground` | `text-gray-500` |
| backing, hover | `bg-muted`, `bg-accent` | `bg-zinc-100` |
| main action | `bg-primary` (through `Button`) | `bg-blue-600` |
| growth, income, done | `text-success`, `bg-success/10` | `text-emerald-600 dark:text-emerald-400` |
| "pay attention": near a limit, stale | `text-warning`, `bg-warning/10` | `text-amber-600` |
| decline, expense, error, delete | `text-destructive`, `bg-destructive/10` | `text-red-500` |
| border | `border-border` (default for `*`), field — `border-input` | `border-gray-200` |

- The light and dark themes take values from `:root` and `.dark`, so manual
  `dark:` colors are not needed. A new meaning is needed — add a variable to
  both themes and `--color-*` to `@theme`.
- The colors of workflow editor nodes and ports are their own tokens
  (`--node-*`, `--port-*`); do not take them for the rest of the interface.
- Charts take their colors from `shared/lib/echarts-theme.ts`; do not write
  hex in chart options.
- Color as **data**, not decoration (the color of a wallet the user chose and
  that is stored in the vault; a fixed graph palette by kinds of operations)
  is an allowed hex. The file declares this once on the first line with a
  reason: `// check-ui-ignore raw-color: <why this is data>`. Without a reason
  after the colon the directive has no effect.

```vue
<!-- Wrong -->
<span class="text-emerald-600 dark:text-emerald-400">+1 200 ₽</span>
<!-- Right -->
<PnlText amount="1200" currency="rub" />
<span :class="signClass(delta)">{{ formatPct(delta, { signed: true }) }}</span>
```

## Text

| Level | Class | Where |
|---|---|---|
| page heading | `heading-page` (18px, 600) | the shell's `h1`, `PageHeader` |
| section heading | `heading-section` (16px, 600) | a page block, a settings window section |
| card heading | `heading-card` (14px, 600) | `SettingsSection`, a `Card` heading |
| big number | `figure` (20px, 600, tabular) | `StatCard`, a widget total |
| main text | `text-sm` | almost everything |
| caption, hint | `text-xs text-muted-foreground` | under a field, row meta |

- A heading is a semantic tag (`h1`…`h3`), not `<p class="font-medium">`.
- The weight does not change on hover or for the selected item: the row must
  not jump.
- Arbitrary sizes (`text-[11px]`) only in dense charts and badges where the
  scale does not fit.

## Spacing

The Tailwind scale (1 = 4px). Conventions:

| Where | Class |
|---|---|
| the shell's margin around a page | `p-3 md:p-4` (set in `AppShell`, a page adds none of its own) |
| between page sections | `flex flex-col gap-4` |
| inside a card | the padding comes from `Card` (`size="sm"` — `p-3`, `md` — `p-3 sm:p-4`); between blocks inside — `gap-3` |
| between form fields | `gap-4` |
| label -> field | `gap-1.5` (given by `FormField`) |
| icon <-> text in a button | `gap-2` (given by `Button`) |

- `gap-*`, not `space-y-*` and not `margin` between neighbors.
- The same blocks on different screens get the same spacing: copy from a
  neighboring screen, do not pick.

## Radii

`--radius` = 10px. `rounded-md` — fields and buttons (set by the components),
`rounded-lg` — nested blocks inside a card, `rounded-xl` — `Card` and
`Dialog`, `rounded-full` — dots and avatars. A nested block is not rounder
than its parent.

## Theme

- `useTheme()` (`shared/lib/theme.ts`) puts `.dark` and `color-scheme` on
  `<html>`; `Sonner` gets the theme from `App.vue`.
- Check the screen in both themes: the dark one is not an "inverted light
  one", the contrast of secondary text is lower there.
