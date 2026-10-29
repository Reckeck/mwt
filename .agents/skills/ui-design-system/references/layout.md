# Layout: page, dialog, Sheet, tabs, narrow windows

## Page

```vue
<template>
  <div class="flex flex-col gap-4">
    <PageHeader :crumbs="[{ label: t('nav.stores'), to: { name: 'stores' } }, { label: store.name }]">
      <template #actions>
        <Button variant="outline" @click="refresh">{{ t('actions.refresh') }}</Button>
        <Button @click="addOpen = true">{{ t('tx.add') }}</Button>
      </template>
    </PageHeader>
    <SectionState …>…</SectionState>
  </div>
</template>
```

- A page in `pages/` is composition only (rules 9-10 of AGENTS.md).
- There is one screen heading: the shell's `h1`. `PageHeader` provides the
  breadcrumbs and actions; its own `title` only if the page refines the shell
  (a wallet name).
- Actions are on the right of the header, the main one is last and filled
  (`default`), the rest `outline`/`ghost`. More than three — the extras go to
  an `ActionMenu` ("More").
- One action — one place on the screen. The same button in the header, in the
  card and in the menu is confusing.
- Gaps between blocks (cards, sections, form rows) — the scale 8/12/16/20/24:
  `gap-2/3/4/5/6`, the same on desktop and phone (no `max-md:gap-*` on a page).
  `gap-2.5`, `gap-3.5` do not exist between blocks.
- A list that may be long (the feed on the Dashboard, a journal) is a block of
  fixed height with scrolling inside (`max-h-* overflow-y-auto`); on a phone
  the page scrolls, there is no inner scroll.
- The margin around a page is set by the shell (`p-3 md:p-4`); the page does
  not add its own.

## Dialog, Sheet or page

| Content | Where |
|---|---|
| one short form or a choice | `Dialog` / `FormDialog` |
| record details at the side, with the list in the background | `Sheet` |
| many sections, navigation, long work | a page (route) |

- **One action — one dialog.** Two buttons that do different things open two
  dialogs; a handover is closing one and opening the other.
- A dialog is not nested in a dialog.
- Inside a dialog there is no `Card` and no second heading; the main button is
  in the footer on the right.
- Width (`dialog.constants.ts`): `sm` = `max-w-sm` (a question, one field),
  `md` = `max-w-lg` (an ordinary form, the default), `lg` = `max-w-2xl` (a
  two-column form), `xl` = `max-w-4xl` (a table, an editor).
- Tabs inside a dialog — only in the settings window. A multi-step wizard is
  steps (`stepper`), not tabs.
- `Dialog`, `Sheet` always have a title (it may be `sr-only`) and, if an
  explanation is needed, a `description`.
- Field layout in a dialog uses container queries (`@xl:grid-cols-2`; sizes:
  Tailwind `@xl` = 36rem, `@2xl` 42, `@3xl` 48, and our own `@mid` = 44rem,
  `@wide` = 52rem); the portal is outside the page, so a `@container` is
  needed (`FormDialog` has one).

## Tabs

- Section tabs are routes (`/planning/budgets`), so that the link and "Back"
  work. Tabs inside a page go in a query parameter (`?tab=history`).
- The order of tabs matches the order of routes and does not change with the
  kind of record without a reason.

## Narrow windows

- The application window can be 800px or smaller (Android). Check at ~800, 390
  and 360; phone rules — [mobile.md](mobile.md).
- No fixed widths on filter fields (`w-56`): `min-w-*` + `flex-wrap`, rare
  filters go in the "More filters" popover.
- Long text is `truncate` (with `min-w-0` on the flex parent) and a `title`
  with the full value.
- Card grids are `grid gap-4 sm:grid-cols-2 lg:grid-cols-4`; inside
  components — container queries.
- On mobile the side menu is a `Sheet` (`Sidebar` does this).
