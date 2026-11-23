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

### Redesign tokens (part A)

The palette is warm paper and ink; `--primary` is ink (no longer blue). Hex values live in `:root` / `.dark`
of `src/app/styles.css`; a vitest (`src/app/styles.test.ts`) keeps WCAG AA for text on canvas and surface.

| New name | Utility | Use |
|---|---|---|
| `--surface-inverse`, `--inverse-foreground` | `bg-inverse`, `text-inverse-foreground` | toast, inverted surface |
| `--success-soft`, `--warning-soft`, `--destructive-soft` | `bg-success-soft` ... | tinted chip/row backgrounds |
| `--scrim` | `bg-scrim` | dialog overlay |
| `--text-tertiary` | `text-tertiary` | placeholders only (3.5:1). Deviation from the spec: Light is `#7A807B` (spec `#7B817C`) and Dark `#767D78`, nudged so the vitest reaches 3.5:1 on canvas and surface |
| `--border-strong` | `border-strong` | emphasised borders |
| `--chart-actual`, `-reference`, `-bank`, `-crypto`, `-property`, `-accent`, `-income`, `-over`, `-grid`, `-band`, `-wash`, `-heat-0..4` | `bg-chart-*` | read by `chart-theme.ts` |

Old to new: `--background` white to canvas `#F3F0E8` / `#111312`; `--card`, `--popover`, `--sidebar` to surface;
`--muted`, `--secondary`, `--accent` to sunken `#EAE6DB` / `#262A28`; `--primary`, `--ring`, `--sidebar-primary*`
from blue to ink; `--syntax-active-line`/`--syntax-selection` from blue tint to ink tint; `--chart-1..5`
stay until the charts move to the `--chart-*` names. Workflow `--port-*`/`--node-*`/`--wire-flow` are unchanged.

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

An explicit scale (8/12/16/24):

| Class | Size | Where |
|---|---|---|
| `rounded-sm` | 8 | popovers and menus |
| `rounded-md` | 12 | fields and buttons (set by the components) |
| `rounded-lg` | 16 | `Card` and card-like blocks |
| `rounded-xl` | 24 | `Sheet`, `Drawer`, `Dialog` |
| `rounded-full` | - | chips, dots and avatars |

A nested block is not rounder than its parent. Surfaces are flat: shared
components keep `shadow-*` only on popovers and menus.

## Theme

- `useTheme()` (`shared/lib/theme.ts`) puts `.dark` and `color-scheme` on
  `<html>`; `Sonner` gets the theme from `App.vue`.
- Check the screen in both themes: the dark one is not an "inverted light
  one", the contrast of secondary text is lower there.

## Workflow accents (`--port-*`, `--node-*`)

Kept unchanged and theme independent. Checked against the new canvas in `src/app/styles.test.ts`: every accent
(except `--port-flow`, a neutral line) reaches at least 1.8:1 against both the Light and the Dark `--background`.
They are graphical accents (ports, node headers); labels use the text tokens. Note this in the PR.
