import type { ColumnDef, RowData, SortingState } from '@tanstack/vue-table'

declare module '@tanstack/vue-table' {
  // The generics must match the library's declaration, even unused.
  interface ColumnMeta<TData extends RowData, TValue> {
    /** Numbers (amounts, quantities, prices) go right so their places line up. */
    align?: 'left' | 'right' | 'center'
    /** Extra classes for this column's cells, header included. */
    class?: string
  }
}

export interface DataTableProps<TData> {
  // biome-ignore lint/suspicious/noExplicitAny: a column's value type differs per column.
  columns: ColumnDef<TData, any>[]
  data: TData[]
  /** The order a fresh table starts in (newest first, largest first…). */
  initialSorting?: SortingState
  /** Stable row ids keep sorting and selection steady across refetches. */
  getRowId?: (row: TData, index: number) => string
  /** Keeps the header in view while the body scrolls; give it a `containerClass` with a max height. */
  stickyHeader?: boolean
  /**
   * The first column stays put while the table scrolls sideways (a narrow
   * screen): the row keeps its name in view.
   */
  stickyFirstColumn?: boolean
  /** Classes of the scrolling wrapper, e.g. `max-h-[60dvh]` for a sticky header. */
  containerClass?: string
  /** Shown in place of the rows when there are none. */
  emptyText?: string
  /** Rows react to a click (`row-click`) and look clickable. */
  clickable?: boolean
}

export type { ColumnDef, SortingState }
