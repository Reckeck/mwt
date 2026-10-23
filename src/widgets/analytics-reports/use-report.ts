import { formatMonthKey } from '@/shared/lib/format'

/** The query itself lives with the report entity; re-exported for the views here. */
export { useReport } from '@/entities/report'

/** Short month label, «мар 26». */
export function monthLabel(key: string, locale: string): string {
  return formatMonthKey(key, { month: 'short', year: '2-digit' }, locale)
}
