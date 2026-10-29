# Phone

Rule 16 of the skill in full. A phone is a window narrower than 768 px (`md`);
the frontend flag is `useIsPhone()` (`shared/lib/phone`, query `PHONE_QUERY`),
the platform (Android, desktop) — `usePlatform()`.

## Rules

1. **The boundary is `md` (768 px).** Page layout switches by `md:` /
   `max-md:`; inside a component — container queries (`@container`,
   `@wide:`), because a component does not know where it stands.
2. **System bar insets — only `*-safe` and `--safe-*`.** The utilities
   `pt-safe`, `pb-safe`, `px-safe`, `inset-safe`; in `calc` —
   `var(--safe-top)` and so on (Android supplies them through the
   `plugin-insets` plugin, the browser through `env(safe-area-inset-*)`).
   Everything `fixed`/`sticky` at a screen edge uses them. -> `check-ui`
   (fixed-edge)
3. **Height is `dvh`, not `vh` and not `h-screen`.** Inside the shell —
   `h-full`: `AppShell` provides the height. -> `check-ui` (viewport-height)
4. **No actions available only from the keyboard, hover or right click.** A
   row with a `ContextMenu` shows a "⋯" button on touch — an `ActionMenu` with
   the same list in `<span class="pointer-fine:hidden">`; "on hover" buttons
   are `pointer-fine:opacity-0 pointer-fine:group-hover:opacity-100`, a finger
   always sees them.
5. **Touch target >= 40 px** on `pointer: coarse` (tabs, menu items and list
   items already get this from `styles.css`; your own icon button is `size-10`
   on a phone).
6. **A dialog comes from the bottom.** `Dialog`/`FormDialog` on a phone draw a
   `Drawer` themselves with the same API; your own popup panel is also in a
   `Drawer` (`shared/ui/drawer`), not `fixed` by coordinates.
7. **A width of `w-56` and up — only with a prefix** (`sm:w-56`), with a
   ceiling (`min-w-[min(14rem,calc(100vw-2rem))]`) or flexible
   (`min-w-0 flex-1`). A `grid` without columns stretches to the longest line
   — write `grid-cols-1`. -> `check-ui` (fixed-width)
8. **One main action on a phone.** The rest go in `#more` of `PageHeader` (a
   "⋯" menu); a button visible only on desktop is `max-md:hidden`, its menu
   item is `md:hidden`.
9. **Horizontal scroll only inside tables and tab strips.** For a table —
   `DataTable sticky-first-column` (or `sticky left-0 z-[1] bg-card` on the
   first cell); route tabs — `RouteTabs`. The page's `scrollWidth` equals the
   window width.
10. **The header and bottom bar are drawn by the shell.** A full-phone screen
    gets `meta.fullscreen` on the route (`route-meta.d.ts`); then its own
    action row is inside the screen.
11. **Check:** 390 and 360 px with inset emulation
    (`document.documentElement.style.setProperty('--native-inset-top', '32px')`,
    `--native-inset-bottom: 48px`), plus 1280 and 800; light and dark theme.

## System "Back" (Android)

`useBackButton` (the shell) closes the top layer first: an open Reka layer
(dialog, sheet, menu, list, settings window) gets Escape itself; your own
layer over the page (a tour) registers on open through `pushBackLayer(close)`
from `shared/lib/back-stack` and is removed on close. Then — the previous
page; on the Dashboard a second press within 2 s minimizes the application.

## Filters, charts

- `FilterBar`: search or period goes in the `#lead` slot and stays in the row;
  everything else on a phone moves into the "Filters (N)" sheet.
- Charts go through `Chart` from `shared/lib/echarts`: on a narrow chart it
  scrolls the legend and thins the axis labels itself.
