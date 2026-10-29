---
name: ui-design-system
description: >
  "How the Multi Wallet Tracker interface looks and behaves: which shared
  component to take (EmptyState, SectionState, DataTable, FormDialog,
  PageHeader, StatCard, Money/PnlText, SettingsSection, AdvancedFields,
  ConfirmButton), color tokens and the heading scale, forms and field
  validation, a notification for every action (`meta` of useMutation,
  `notify`), loading/error/empty, tables and money, keyboard and focus, copy
  texts. Load before any markup in src/**/*.vue: screen, dialog, form, table,
  button, card, toast, empty state, loading, amount, P&L, dark theme, UI review.
  Триггеры: вёрстка, экран, диалог, форма, таблица, кнопка, карточка, тост,
  пустое состояние, загрузка, сумма, тёмная тема, UI-ревью."
paths: src/**/*.vue
---

# Multi Wallet Tracker UI

The skill answers two questions: **what to build** a screen from and **how it
should behave**. How to add or fix a shadcn-vue component itself — the
`shadcn-vue` skill; where code lives and where to import it from —
`feature-sliced-design` and `frontend-feature`; tours and "What's new" —
`onboarding`.

**Do not apply to:** Rust, CI, pure logic in `lib/`/`model/` without markup.

## How the project differs from the shadcn-vue skill

`shadcn-vue` is installed from the registry and is not edited, so the
differences are recorded here. On these points the project wins:

| shadcn-vue says | In the project |
|---|---|
| icons `@lucide/vue` | `lucide-vue-next`; after `shadcn-vue add` replace the import |
| deletion is confirmed by `AlertDialog` | `ConfirmButton` in place (click -> "Delete?" -> click), for something reversible — "Undo" in the notification |
| forms on `FieldGroup` + `Field` | `FormField` (a wrapper over shadcn `Field`: `label`, `required`, `error`, `hint`); `FieldSet`/`FieldGroup` around it are fine |
| `CardHeader`/`CardContent`/… | `Card` with slots `#header` / default / `#footer` (the parts are marked with `data-slot`) |
| `Button` has no loading state, a `Spinner` next to it | the same; in a form dialog `FormDialog` already does it |
| the widgets layer is not needed (FSD skill) | widgets exist — rule 10 of AGENTS.md |

shadcn components live in `src/shared/ui/<name>/` (the `components.json`
aliases). The CLI cannot do a dry run: install into a temporary copy of the
project and carry over the needed folders, so as not to overwrite the
customized `button`, `input`, `label`, `dialog`.

## Critical rules

Each rule says what checks it. "Review" means: there is no script, watch for it
yourself.

1. **A ready component instead of markup.** First the catalog below, then the
   shadcn-vue registry, only then your own composition of shadcn components in
   `shared/ui`. Similar markup in three places is a reason to extract a shared
   component. -> `check-ui` (raw-button, raw-check, raw-table, hand-empty)
2. **Every action reports its outcome.** `useMutation` has
   `meta: { success, error }` (and `undo` for something reversible) or
   `meta: { silent: '<why>' }` — then the error is shown by its own `onError`
   through `notify.error`. The notification is shown by `MutationCache`, not by
   the component. -> `check-ui` (mutation-meta),
   [feedback.md](references/feedback.md)
3. **Loading, error and "empty" are distinguishable.** A false "Nothing here"
   while the data is still on its way is a bug. `SectionState` by
   `isPending`/`error`/empty. -> review
4. **Money goes through `Money`/`PnlText`/`format.ts`.** Digits are
   `tabular-nums`, right-aligned in a table; P&L has a sign, zero is neutral.
   -> `check-ui` (to-fixed), [data-display.md](references/data-display.md)
5. **Color is not the only carrier of meaning.** Growth/decline — sign and
   color, status — text or icon and color. -> review
6. **Tokens only.** `text-success`, `text-warning`, `text-destructive`,
   `bg-muted`…; no `emerald-*`/`red-*`, hex or manual `dark:`. -> `check-ui`
   (raw-color), [tokens.md](references/tokens.md)
7. **The save button is active**, fields are checked on press and show the
   error under themselves. -> `check-forms`, [forms.md](references/forms.md)
8. **Account, asset, currency — only ready pickers** (rule 14). ->
   `check-forms`
9. **Enter submits the form** (a form dialog is `FormDialog` or `<form>`), Esc
   closes the window, focus is visible (`focus-visible:ring`). -> review,
   [a11y-motion.md](references/a11y-motion.md)
10. **A dangerous action — confirmation in place or "Undo".** -> review
11. **Animation only where it helps understanding**, no `transition-all`;
    `prefers-reduced-motion` turns everything off globally (`styles.css`). ->
    `check-ui` (transition-all)
12. **The screen state survives navigation:** the tab and filters live in the
    URL query parameters. -> review
13. **A long list is not drawn whole:** more than ~50 rows — "Show N more" page
    by page (the sample is `TransactionsList`). -> review
14. **Texts in both locales**, by the rules of [copy.md](references/copy.md).
    -> `check-onboarding` (key match)
15. **A file is one meaningful part:** `.vue` up to 250 lines, `.ts` up to 300;
    longer — split into `model/`, `lib/`, `ui/`. -> `check-ui` (file-size)
16. **Works on a phone:** the boundary is `md` (768 px), system bar insets are
    `*-safe`, no action only from the keyboard, hover or right click. ->
    [mobile.md](references/mobile.md), `check-ui`
17. **Ready-made instead of a cut-down copy.** Before drawing an entity (a
    position, a transaction, a wallet, a price chart, an alert), find in
    `entities/*` and `widgets/*` a component that already shows it and embed it
    — with a prop if something is missing. Any amount with a currency is
    `<Money>` (the rates tooltip), not `formatMoney` in the template. ->
    `check-ui` (money-text, money-lib)
18. **The interface does not move by itself** (rule 17 of AGENTS.md): the place
    for row buttons is reserved, a button is not swapped for another, a query
    keeps the previous data, a subscription is `useBackendEvent`, no
    `backdrop-filter`. -> `check-ui`, `check-layout`,
    [stable-layout.md](references/stable-layout.md)
19. **Favourites in every list of currencies and assets:** a "Favourites" group
    on top and a star. The pickers provide this (rule 8) and so does
    `useFavoriteCurrencies`; you do not write your own list of currencies. ->
    `check-forms`

## Catalog: task -> component

| Task | Component | Where |
|---|---|---|
| page header: breadcrumbs, title, actions | `PageHeader` | `shared/ui/page-header` |
| loading / error with "Retry" / empty / data | `SectionState` | `shared/ui/section-state` |
| an empty state with a "Create" button | `EmptyState` | `shared/ui/empty-state` |
| on/off with a label and explanation (a setting, "Active" in a dialog) | `SwitchRow` | `shared/ui/switch-row` |
| a share: budget, goal, file transfer (a bar) | `ProgressBar` | `shared/ui/progress-bar` |
| a table with sorting, numbers on the right | `DataTable` (`ColumnDef` columns, `meta.align`) | `shared/ui/data-table` |
| label + big number | `StatCard` | `shared/ui/stat-card` |
| an amount with a currency and a rate hint | `Money` (`colored` — sign and color) | `entities/currency` |
| profit/loss, with percent | `PnlText` | `entities/currency` |
| a dialog with one form | `FormDialog` (`form-id`, `pending`, `@submit`) | `shared/ui/form-dialog` |
| a form field with `*`, error, hint | `FormField` | `shared/ui/form-field` |
| a settings block with a single heading | `SettingsSection` | `shared/ui/settings-section` |
| rare form fields under "More options" | `AdvancedFields` | `shared/ui/advanced-fields` |
| date / date and time / month | `DateTimePicker` (`mode`) | `shared/ui/date-time-picker` |
| your own list with search (thousands of rows, two-line rows) | shadcn combobox parts: `ComboboxRoot`, `ComboboxList`, `ComboboxInput`, `ComboboxItem`, `ComboboxGroup`; a long list — reka `ComboboxVirtualizer` (samples — `CurrencyPicker`, `AccountRefPicker`, `AssetInput`) | `shared/ui/combobox` |
| a filter bar for a list or report | `FilterBar` (frequent ones in the slot, rare ones in `#more`, `active` -> "Reset (N)"; the field is `FILTER_FIELD_CLASS`) | `shared/ui/filter-bar` |
| a list row or card that is one big button | `Button size="block"` (the layout itself via classes) | `shared/ui/button` |
| a small "×" in a chip or row | `Button variant="ghost" size="icon-xs"` (`icon-sm` — 32 px) | `shared/ui/button` |
| a table without sorting (months in order, a cell spanning several columns) | shadcn `Table` parts | `shared/ui/table` |
| delete / remove with confirmation | `ConfirmButton` | `shared/ui/confirm-button` |
| a "⋯" menu of a row or card | `ActionMenu` | `shared/ui/action-menu` |
| a Dashboard widget | `WidgetFrame` | `shared/ui/widget-frame` |
| a warning callout, an error in place | `Alert` | `shared/ui/alert` |
| popover, command, a list with search | `Popover`, `Command`, `Combobox` | `shared/ui/…` |
| a choice of 2-7 options | `ToggleGroup` | `shared/ui/toggle-group` |
| radio, checkbox, switch | `RadioGroup`, `Checkbox`, `Switch` | `shared/ui/…` |
| breadcrumbs on their own | `Breadcrumb…` | `shared/ui/breadcrumb` |
| waiting on a button | `Spinner` | `shared/ui/spinner` |
| a loading placeholder | `Skeleton` (usually through `SectionState`) | `shared/ui/skeleton` |
| a hotkey in a hint | `Kbd` | `shared/ui/kbd` |
| account / wallet | `StoreNodeSelect`, in a transaction — `AccountRefPicker` | `entities/store`, `features/transaction-create` |
| asset | `AssetInput` (search), `AssetSelect` (from the known ones) | `entities/asset` |
| currency | `CurrencyPicker` | `entities/currency` |
| a trading pair `BTCUSDT` | `PairInput` (`quotes`, `kind="futures"`) | `entities/asset` |
| chart currency: base or favourite | `ChartCurrencySwitch` | `entities/currency` |
| an amount in the base and favourite currencies | `MoneyConversions` (the body of the `Money` tooltip) | `entities/currency` |
| favourites: read, add, order | `useFavoriteCurrencies`, the button — `FavoriteStarButton` | `entities/currency`, `features/currency-favorite` |
| wallet positions (price, 24h, source, P&L) | `StorePositions` (`clickable`, `@open`) | `entities/store` |
| a whole asset: price, chart, alerts, wallets | `AssetDetailsSheet` | `widgets/asset-details` |
| a sub-account row with a link | `StoreRowLink` | `entities/store` |
| a wallet card | `StoreCard` (the "⋯" menu and right click are one list) | `entities/store` |
| a wallet value chart | `StoreChart` | `entities/store` |
| a price chart with drawing and an alert | `TradingChart` (`can-alert`, `@alert`) | `widgets/trading-chart` |
| an alert: create / list | `PriceAlertDialog` (`preset`), `PriceAlertList` (`asset`, `preset`) | `features/price-alert-edit`, `widgets/price-alert-list` |
| shares: a ring | `AllocationPie` (`slices`, `bare`) | `widgets/allocation-pie` |
| open a transaction card | `openTxDetails(tx)` (one sheet in the shell) | `shared/lib/tx-details` |
| currency converter | `openConverter(code)`, inside a page — `ConverterPanel` | `shared/lib/converter`, `features/currency-convert` |
| a list intro + a "Create" button | `ListHeader` | `shared/ui/list-header` |
| "Available: … · All" under an amount | `AvailableHint` | `features/transaction-create` |
| desktop or phone | `usePlatform()` (`isDesktop`, `isMobile`, `isAndroid`) | `shared/lib/platform` |
| a phone window (< 768) | `useIsPhone()` | `shared/lib/phone` |
| a bottom sheet | `Drawer` (`Dialog` on a phone is already one) | `shared/ui/drawer` |
| tabs = child routes | `RouteTabs` (`keep-query`) | `shared/ui/route-tabs` |
| rare page actions | `PageHeader` slot `#more` | `shared/ui/page-header` |
| your own layer over the page and "Back" | `pushBackLayer(close)` | `shared/lib/back-stack` |

The full list is the `src/shared/ui/*` folders (each with an `index.ts`). If
what you need is missing — `pnpm dlx shadcn-vue@latest search`, then `add`.

## Order of work

1. **Check it does not already exist.** Search for the IPC command and the i18n
   key (`grep -rn "delete_contact" src`): the action may already live in
   another feature.
2. **Check the backend contract** before laying out a form: the DTO in
   `ipc.gen.d.ts` and the checks in `crates/core/src/service/*` — required
   fields and refusals that the form must catch itself (rule 13).
3. **Build from the catalog.** The catalog wins over existing code: old
   screens are still being moved to the shared components
   (`check-ui.baseline.json`), so "do it like the neighboring file" may lead
   to an old template. Your own — only a composition in `shared/ui` (a folder
   + `index.ts` + `*.types.ts`, rule 11).
4. Wire up the outcome of every action (`meta`), the three data states
   (`SectionState`), texts in both locales.
5. Onboarding (the `onboarding` skill): a new screen — a tour in `tours.ts`; a
   new noticeable element — a step in its screen's tour and `data-tour`; a
   behavior change — an item in the current `whats-new.ts` entry.
6. Check with your eyes in `pnpm dev` (mock, password `secret`): light and dark
   theme, a 1280 window and a narrow ~800, a phone at 390 and 360 with inset
   emulation ([mobile.md](references/mobile.md)), loading (no false "empty").
7. `mise run check` (Biome, vue-tsc, `check-onboarding`, `check-forms`,
   `check-ui`, clippy, tests).

## Checks

| Script | Catches |
|---|---|
| `mise run check-ui` | raw `button`/`checkbox`/`radio`/`table`, a hand-made empty state, `emerald`/`red`/hex, `transition-all`, `toFixed` in `.vue`, `div @click` without a role, `formatMoney` in a template, `h-screen`/`vh`, `fixed` at an edge without `*-safe`, a wide width without a prefix, `useMutation` without `meta`, a file longer than the limit |
| `mise run check-forms` | a save button blocked by field validation, a manual check in `@submit`, home-made lists of accounts/assets/currencies |
| `mise run check-onboarding` | a screen without a tour, a step without an element, no text, ru keys != en |
| `mise run check-layout` | the application in headless Chromium: sideways scroll, overflow, shift on hover and loading, dialog jump, listener leaks, "Back", touch targets |

Debts that existed before the rule are recorded in
`scripts/check-ui.baseline.json`. A file cannot get worse than its entry. Fixed
one — `mise run check-ui -- --update`, the entry will shrink. A new file must
be clean.

## UI review: report format

Findings by file, `file:line`, one line per finding, first what hinders the
user (a false "empty", a silent action, a button unreachable by keyboard), then
consistency. No preamble.

## Reference

- [tokens.md](references/tokens.md) — colors, the heading scale, spacing, radii, dark theme
- [forms.md](references/forms.md) — rules 13 and 14 in full, `useFormValidation`, `FormDialog`
- [feedback.md](references/feedback.md) — notifications, loading, errors, empty, confirmation and "Undo"
- [mobile.md](references/mobile.md) — rule 16: phone, insets, "Back", sheets, widths
- [stable-layout.md](references/stable-layout.md) — rule 17: nothing jumps on hover, loading, scrolling
- [data-display.md](references/data-display.md) — tables, money, percents, dates, P&L, charts, long lists
- [layout.md](references/layout.md) — page, dialog, Sheet, tabs, narrow windows
- [a11y-motion.md](references/a11y-motion.md) — keyboard, focus, ARIA, contrast, animation
- [copy.md](references/copy.md) — interface texts in ru and en, the glossary

Where the rules come from: Vercel Web Interface Guidelines, the
`unovue/shadcn-vue` skill, Anthropic Agent Skills best practices, Rauno
Freiberg "Interfaces", Pencil & Paper — data tables (links are in
`references/`).
