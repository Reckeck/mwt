# Stable layout (rule 17)

The interface does not move by itself: neither hover, nor focus, nor a press,
nor data loading, nor a scrollbar appearing changes the size and place of its
neighbors. Every item below is a real bug that was already fixed (audit
2026-10-20).

Checks: `mise run check-ui` (static, the rules are in
`tests/checks/rules/ui/`) and `mise run check-layout` (the application on the
mock in headless Chromium).

## Hover and focus

- **Row buttons go in a reserved column.** The place for actions is always
  there (a fixed width or a "⋯" `ActionMenu`); on hover only `opacity`
  changes. Lazy mounting is fine — but into an already occupied place.
  -> `hover-mount`, `check-layout` ("row moves Npx on hover/leave")
- **Hidden until hover — for the mouse only:** `pointer-fine:opacity-0
  pointer-fine:group-hover:opacity-100`. Without `pointer-fine:` a finger will
  not see the button. -> `hover-only`, `check-layout` ("invisible on touch")
- **A tooltip does not recreate its trigger.** A heavy body
  (`MoneyConversions`) is lazy inside the open `TooltipContent`; the open
  delay is shared (300 ms), zero only for the collapsed sidebar's menu.

## A control changes state, not itself

- **One element.** The "Delete?" of `ConfirmButton` is the same `<button>` of a
  different variant and the same width; focus stays on it. A pair
  `<Button v-if>` / `<Button v-else>` destroys focus and jumps in width.
  -> `control-swap`, `check-layout` ("confirm button changes width / loses
  focus")
- **A spinner replaces the icon, it does not go before the text.** A pending
  button is no wider than usual: the left icon is swapped for a `Spinner` of
  the same size, or the spinner goes over the text. -> `control-swap`

## Data

- **Changing a key does not empty the screen.** A query with
  `queryKey: computed(…)` gets `placeholderData: keepPreviousData`; the
  previous data is visible (dimmed) while the new data arrives. A key of
  "entity + filter" (account and period) — `keepPreviousFor(id)` from
  `shared/lib/query`: the filter changes without a gap, and another account is
  not shown under the new heading. A key that is the entity only, or data that
  fills a form via `watch` — no placeholder, with a comment
  `// no placeholder: <why>` before `queryKey`. -> `query-keep-data`,
  `check-layout` ("layout shift")
- **`SectionState`:** the first load is a skeleton of content height right
  away; a refresh keeps the content. An empty wrapper of zero height is a bug.
- **Subscribing to a backend event is `useBackendEvent`** (removed with the
  scope). A bare `onEvent` in `setup` adds a handler on every visit. A timer
  with a handle — `clearTimeout` in `onScopeDispose`. -> `raw-listener`,
  `check-layout` ("listeners grow per visit")
- **An event invalidates only its own keys.** `invalidateQueries()` without a
  key redraws all screens — only when the whole vault is replaced (with
  `check-ui-ignore invalidate-all: <why>`). -> `invalidate-all`
- **A page is not recreated when the path changes.** `RouterView` without
  `:key`; the page reacts to `route.params` through query keys. ->
  `router-key`
- **Formatters come from `shared/lib/format`** (an `Intl` cache), not
  `new Intl.*` per call. -> `intl-cache`
- **A list row computes itself once:** the row model in a `computed` or a row
  component, not `fn(row)` three times in the template. -> `row-repeat-call`

## Scrolling and sizes

- **A scroller has `scrollbar-gutter: stable`:** the scrollbar appearing does
  not shift the right edge.
- **A grid with fixed tracks** (`grid-cols-[1fr_8rem_9rem]`) from 14rem — only
  with a breakpoint; in a narrow container the row folds into two. ->
  `grid-fixed-track`
- **`--container-*` in `@theme` are not overridden:** in Tailwind v4 they also
  define `max-w-sm/md/lg`. The project's container-query sizes have their own
  names: `@mid` (44rem), `@wide` (52rem); 36rem is the standard `@xl`.
  Dialogs: sm < md < lg < xl = `max-w-md/2xl/3xl/4xl`. ->
  `theme-token-override`
- **Scales:** `text-2xs` instead of `text-[10px]`; `z-sticky / z-overlay /
  z-popover` instead of `z-50`/`z-[60]`. -> `arbitrary-size`, `z-scale`

- **The window minimum is the phone one (360×480), no higher.** A tiling
  compositor (Hyprland, Sway) does not honor `minWidth`: a window in a
  566 px tile with a minimum of 768 was drawn at 768 and squeezed as a
  picture — the interface "shrank" instead of reflowing. A narrow window gets
  the phone layout with a mouse; `check-layout` checks this at 560×860.

## Animation and WebKitGTK

- **An overlay animates `opacity` and `scale`.** Tailwind v4 centers through
  the `translate` property; `transform: translate(-50%,-50%)` in the frames
  adds a second shift. Animations go through `@utility`, otherwise the
  `data-[state=open]:` variant is not generated. -> `transform-keyframes`
- **No `backdrop-filter`.** WebKitGTK (the Linux window) recomputes every blur
  layer on a frame; on a list this froze the application. An opaque
  `bg-card`/`bg-popover`. -> `backdrop-blur`
- **In the Linux window overlays open without animation.** WebKitGTK on
  Wayland repaints the whole window when an animated layer appears: every
  dialog, sheet, menu and tooltip flickered. `index.html` sets
  `data-engine="webkitgtk"`, and `styles.css` turns off `animation` there for
  everything with `[data-state]`, and toast transitions. A new overlay does
  nothing for this; do not add animations to it bypassing `data-state`.
- **Do not animate `width/height/left/top/margin/padding`** — it relayouts the
  page on every frame; use `transform`/`opacity`. -> `layout-transition`

## How to check by hand

`mise run check-layout` writes screenshots of all screens to
`.cache/check-layout/<theme>-<width>-<view>.png`. Headless Chromium by itself
reports `pointer: none`; the stand launches it with a `fine` pointer and
`hover`, like WebKitGTK, and the phone sizes with touch emulation. The final
check is the Tauri window on Linux (`mise run dev`): run the mouse over the
rows and leave, change the period in analytics, open five dialogs, quickly
toggle three settings.
