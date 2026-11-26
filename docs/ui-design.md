# UI design

A short overview for people. Rules with code examples and checks live in the
[`ui-design-system`](../.agents/skills/ui-design-system/SKILL.md) skill and its
[references](../.agents/skills/ui-design-system/references/); that is also
the single source of numbers (spacing, type scale, dialog widths).
If this document and the skill disagree, the skill is right.

## Principles

1. **One thing — one component.** Screens are built from
   [shadcn-vue](https://shadcn-vue.com) components and the project's shared components
   (`src/shared/ui`): there is one table everywhere, one empty state, one form
   dialog. Repeated markup is extracted into a shared component.
2. **Every action responds.** Saved, deleted, imported —
   a notification in the corner of the window; an error says why and how to fix it;
   anything reversible can be undone right from the notification.
3. **Loading is not emptiness.** While data is on its way a placeholder is shown; if it did not
   arrive, the reason and "Retry"; "Nothing here" only when there really is
   nothing, and with a button saying where to start.
4. **Money reads the same everywhere.** One formatting for amounts, percentages,
   quantities and dates; profit with a sign, zero is not green; digits in even
   columns.
5. **The keyboard is a first-class way.** Enter saves, Esc closes,
   focus is visible, every action is reachable without a mouse.
6. **A calm interface.** Color carries meaning (growth, attention, error), not
   decoration; animation only where it explains a transition, and it fades out with the
   system "reduce motion".
7. **Onboarding together with the feature.** A tour, in-place hints, "What's new" —
   the [`onboarding`](../.agents/skills/onboarding/SKILL.md) skill.

## Screen layout

```
┌ sidebar ──────┬ header: screen title · price status · total · + Transaction · 🔔 · ? ┐
│ sections      │ breadcrumbs › path                         screen actions →          │
│ wallets       │ content: sections via gap-4, cards, tables, charts                   │
│ settings      │                                                                       │
└───────────────┴───────────────────────────────────────────────────────────────────────┘
```

- The sidebar holds the app sections and wallets by kind; in a narrow window it
  slides in over the content.
- The screen header is `PageHeader`: where I am (breadcrumbs), what I do (actions, the main one
  last).
- Record details open on the side (`Sheet`), short forms in a dialog,
  large sections as a separate page.

## Themes

30 built-in themes (Default light/dark, Dracula, Nord, Gruvbox, Catppuccin and others) are chosen
in Settings. The choice is per device; "Auto" pairs one light and one dark theme and follows the
system. A theme changes colours, money colours, chart palette and the radius only: never the font,
spacing or layout.

- **Tokens.** A theme owns the CSS custom properties listed in `THEME_TOKENS`
  (`src/shared/lib/theme/tokens.ts`): surfaces, text, primary/secondary/accent, status colours with
  their soft variants, borders, sidebar, charts, the four money colours (`--money-income`,
  `--money-expense`, `--money-transfer`, `--money-in-transit`) and `--radius`. Components use the
  tokens, never raw colours (`check-ui`).
- **Add a theme.** Create `src/shared/lib/theme/themes/<id>.ts` exporting a `Theme` (`id`, `name`,
  `mode`, a complete `tokens` map; the header names the source palette, its licence and every token
  tuned away from upstream as `tuned from upstream: <token>, <original value>`), then add it to
  `builtinThemes` in `registry.ts`. Run `pnpm exec vitest run src/shared/lib/theme`: the registry,
  `assertCompleteTheme` (every token present) and the contrast test must pass.
- **Contrast test.** `themes.contrast.test.ts` checks every built-in theme against
  `CONTRAST_PAIRS` (`contrast-pairs.ts`): text pairs at 4.5:1 (WCAG AA), chart and graphic pairs at
  3:1. A failing palette is tuned (the nearest passing colour) and the change is noted in the
  header; the only exceptions are the four Default chart tokens listed in the test.
- **Look at it.** `mise run shots -- themes --theme <id|all>` renders five screens at desktop and
  phone widths into `.cache/shots/themes/`, and checks that the page really shows the picked theme's
  background. It is not a gate: open the PNG files and compare colours with the tokens.

## Checks

`mise run check` includes the interface checks:
`check-ui` (components, tokens, notifications, file size),
`check-forms` (forms and account/asset/currency pickers),
`check-onboarding` (tours and texts).
