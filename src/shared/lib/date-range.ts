import { dateFormat } from '@/shared/lib/format'

/** A day or a span of days as ISO `YYYY-MM-DD` strings; `end` is empty for a single day. */
export interface DateRange {
  start: string | null
  end: string | null
}

const ISO = /^(\d{4})-(\d{2})-(\d{2})/

function parse(value: string | null | undefined): Date | null {
  const match = ISO.exec(value ?? '')
  if (!match) return null
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]))
}

/** `2 Nov` in English (ru: the first three letters of the genitive month). */
function dayMonth(date: Date, locale: string): { day: string; month: string } {
  const day = String(date.getDate())
  if (locale.startsWith('ru')) {
    const long = dateFormat({ day: 'numeric', month: 'long' }, locale).format(date)
    return { day, month: long.replace(/^\d+\s*/, '').slice(0, 3) }
  }
  const short = dateFormat({ month: 'short' }, locale).format(date)
  return { day, month: short.replace(/\.$/, '') }
}

/**
 * How a range reads in a button: `Nov 2`, `Nov 2–4`, `Oct 30 – Nov 2`; a span across
 * years adds the year to both ends. Empty for no start.
 */
export function formatDateRange(range: DateRange | null | undefined, locale: string): string {
  const start = parse(range?.start)
  if (!start) return ''
  const end = parse(range?.end)
  const first = dayMonth(start, locale)
  const year = (date: Date) => ` ${date.getFullYear()}`
  if (!end || end.getTime() === start.getTime()) return `${first.day} ${first.month}`
  const last = dayMonth(end, locale)
  if (start.getFullYear() !== end.getFullYear()) {
    return `${first.day} ${first.month}${year(start)} – ${last.day} ${last.month}${year(end)}`
  }
  if (start.getMonth() === end.getMonth()) return `${first.day}–${last.day} ${first.month}`
  return `${first.day} ${first.month} – ${last.day} ${last.month}`
}

/** True when `end` is before `start` (both set). ISO strings compare as text. */
export function isRangeReversed(range: DateRange): boolean {
  return Boolean(range.start && range.end && range.end < range.start)
}
