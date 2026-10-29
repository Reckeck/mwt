import { dateFormat, formatDayMonthShort } from '@/shared/lib/format'

/** `Oct 21, 18:00`: when the exchange was last imported, in its status line. */
export function formatDayMonthTime(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  const time = dateFormat({ hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(date)
  return `${formatDayMonthShort(iso)}, ${time}`
}
