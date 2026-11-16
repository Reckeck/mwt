/** Date-time input helpers shared by forms. */

import { formatDate } from './format'

/** `datetime-local` (local time, no zone) ⇄ RFC3339 UTC. */
export function toIsoTimestamp(local: string): string | null {
  if (!local) return null
  const parsed = new Date(local)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
}

/** `YYYY-MM-DD` of the user's own calendar day (not the UTC one): what "today" means to them. */
export function localDay(moment: Date = new Date()): string {
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${moment.getFullYear()}-${pad(moment.getMonth() + 1)}-${pad(moment.getDate())}`
}

/**
 * A calendar day (`YYYY-MM-DD`, no zone) as `Oct 20, 2026`. It is read at local noon, so no zone moves it to a
 * neighbouring day (`T00:00:00Z` would show the day before west of Greenwich).
 */
export function formatCalendarDay(day: string | null | undefined): string {
  return day ? formatDate(`${day}T12:00:00`) : '—'
}

export function toLocalInput(iso: string): string {
  const parsed = new Date(iso)
  if (Number.isNaN(parsed.getTime())) return ''
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())}T${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`
}

/**
 * The moment to save from a `datetime-local` field that shows an existing
 * moment to the minute: when the user left the field as it was, the original
 * (with its seconds) is kept, so re-saving an operation never moves it in
 * time — not even past another operation within the same minute.
 */
export function keepOriginalMoment(local: string, original: string | null | undefined): string | null {
  if (original && toLocalInput(original) === local) return original
  return toIsoTimestamp(local)
}
