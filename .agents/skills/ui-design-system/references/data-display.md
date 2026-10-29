# Data: tables, money, P&L, charts, lists

## Numbers

| What | How to show it |
|---|---|
| amount | `<Money :amount :currency>`; in a string — `formatMoney(value, { currency })` |
| amount with sign and color | `<Money colored>` |
| profit/loss | `<PnlText :amount :currency :pct>` |
| percent | `formatPct(value)`, signed — `formatPct(value, { signed: true })`; the value is **already in percent** (`12.5` -> "12.50 %" in en, "12,50 %" in ru), not a fraction |
| integer (trades, pieces) | `formatQuantity(value)` |
| asset quantity | `formatQuantity(value)` — without trailing zeros (`1`, `0.00042`) |
| unit price | `formatPrice(value, currency)` — 2 decimals from 1 and up, more precise for small ones |
| date / date and time | `formatDate`, `formatDateTime`; "today at 14:05" — `formatAsOf` |
| color by sign | `signClass(value)` / `SIGN_TONE_CLASS[signTone(value)]` |

- No `toFixed`, `toLocaleString(undefined)` or a hand-typed "−": the interface
  locale and digit grouping exist only in `format.ts`.
- Money comes from Rust as a string (`Amount`); do not turn it into `f64` for
  logic — only for display.
- The same quantity is formatted the same way on different screens (a
  wallet's total on the card, on the page and in the report — the same number
  of decimals).
- Zero is neutral: not "profit" and not green.
- Digits that are compared by eye are `tabular-nums` (already in `Money`,
  `StatCard`, the right-hand columns of `DataTable`).

## Table — `DataTable`

Columns are a `computed`: headers via `t()` must change together with the
language.

```ts
const columns = computed<ColumnDef<Position>[]>(() => [
  { accessorKey: 'symbol', header: t('pos.asset') },
  {
    id: 'qty',
    header: t('pos.qty'),
    accessorFn: (row) => Number(row.quantity),
    cell: ({ row }) => formatQuantity(row.original.quantity),
    meta: { align: 'right' },
  },
  {
    id: 'pnl',
    header: t('pos.pnl'),
    accessorFn: (row) => Number(row.pnl_base),
    cell: ({ row }) => h(PnlText, { amount: row.original.pnl_base }),
    meta: { align: 'right' },
  },
])
```

```vue
<DataTable :columns="columns" :data="rows" :initial-sorting="[{ id: 'pnl', desc: true }]" sticky-header />
```

- Numbers are right-aligned (`meta.align: 'right'`), text and dates are
  left-aligned, nothing is centered. The header is aligned like its column.
- Sorting by clicking the header; a sensible default sort (new on top, large
  on top).
- Thin row dividers, no zebra; the header is pinned if the table scrolls.
- Empty because of a query (no data at all, none for the period) —
  `SectionState :empty` around the table; the table's `emptyText` is only for
  emptiness inside an already shown table (for example, after a local filter
  by a search string).
- Where it lives: a report or multi-entity screen table — `widgets/`; a
  single-entity table without actions — `entities/<name>/ui`.
- A link row: `clickable` + `@row-click` (keyboard: Tab + Enter already work).
- Row actions — `ActionMenu` in the last column.

## Long lists

More than ~50 rows — page by page with a "Show N more" button
(`ui.showMore`), as in `TransactionsList` (100 at a time, a new filter starts
again from the first page) and `LossSummary`. Virtualization
(`@tanstack/vue-virtual`) only for rows of equal height: rows of different
heights jump in a virtual list. Truncation without a button ("first 8") hides
data silently.

A repeating list item (a row, a day header, anything inside `v-for`) has no
`backdrop-blur` / `backdrop-filter`, and an opaque background (`bg-card`): a
dozen such layers in a scroll and WebKitGTK (the Linux window) stops drawing
frames — the application hangs for good. Blur only on single elements (a
header, a chart legend, a bottom bar).

## Charts (vue-echarts)

- Height goes on the wrapper: `<div class="h-80"><Chart … /></div>` (vue-echarts
  8 sets `height: 100%` as a style, which overrides `h-*` on `<Chart>`
  itself).
- A new chart type — registration in `shared/lib/echarts.ts`.
- Every chart has: axis labels through the money formatter, a tooltip with the
  currency, a legend with several series, a "No data" state instead of empty
  axes (`SectionState :empty`).
- Series colors come from the theme (`echarts-theme.ts`), distinguishable with
  color blindness; growth/decline additionally differ by shape or label.
- The chart theme follows the app theme (reactively, not once at creation).

Sources: Pencil & Paper "Enterprise data tables"
(pencilandpaper.io/articles/ux-pattern-analysis-enterprise-data-tables),
Vercel Web Interface Guidelines (github.com/vercel-labs/web-interface-guidelines).
