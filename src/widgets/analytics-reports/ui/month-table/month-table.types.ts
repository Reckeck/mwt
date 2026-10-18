export interface MonthTableColumn {
  key: string
  label: string
  /** What the column means, on hover of its name. */
  hint?: string
  /** Shown on a narrow screen too; the rest open under the row. */
  primary?: boolean
  /** Coloured by sign. */
  signed?: boolean
  /** Formats the cell; money without decimals by default. */
  format?: (value: string) => string
}

import type { RouteLocationRaw } from 'vue-router'

export interface MonthTableRow {
  id: string
  label: string
  values: Record<string, string>
  /** Greyed out, cells shown as «—». */
  muted?: boolean
  bold?: boolean
  /** The transactions behind the row. */
  to?: RouteLocationRaw
}

export interface MonthTableProps {
  columns: MonthTableColumn[]
  rows: MonthTableRow[]
  /** Header of the first column. */
  labelHeader: string
}
