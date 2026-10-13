/** Date-time input helpers shared by forms. */

/** `datetime-local` (local time, no zone) ⇄ RFC3339 UTC. */
export function toIsoTimestamp(local: string): string | null {
  if (!local) return null
  const parsed = new Date(local)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
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
