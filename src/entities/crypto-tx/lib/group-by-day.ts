import { decimalAdd } from '@/shared/lib/decimal'
import { localDay } from '@/shared/lib/datetime'

export interface OpDay {
  /** `YYYY-MM-DD` of the user's own calendar day, the one the time in the row shows. */
  date: string
  rows: CryptoOpRow[]
  /** USD value of what has a price, as an exact decimal string; `null` when none has. */
  total: string | null
}

/** Rows arrive newest first; a new day starts where the local calendar day changes. */
export function groupByDay(rows: readonly CryptoOpRow[]): OpDay[] {
  const out: OpDay[] = []
  for (const row of rows) {
    const moment = new Date(row.time)
    const date = Number.isNaN(moment.getTime()) ? row.time.slice(0, 10) : localDay(moment)
    let day = out.at(-1)
    if (!day || day.date !== date) {
      day = { date, rows: [], total: null }
      out.push(day)
    }
    day.rows.push(row)
    if (row.value_usd) day.total = decimalAdd(day.total ?? '0', row.value_usd) ?? day.total
  }
  return out
}
