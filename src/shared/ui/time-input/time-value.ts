/** The text being typed into a time field: digits only, a colon after the hours. */
export function maskTime(text: string): string {
  const digits = text.replace(/\D/g, '').slice(0, 4)
  return digits.length > 2 ? `${digits.slice(0, 2)}:${digits.slice(2)}` : digits
}

/**
 * A finished time «HH:MM» (24 hours) from what was typed, or `null` when it
 * is not a time: «9» → 09:00, «930» → 09:30, «9:5» → 09:05, «25:00» → null.
 */
export function parseTime(text: string): string | null {
  const trimmed = text.trim()
  if (!trimmed) return null
  let hours: string
  let minutes: string
  const parts = /^(\d{1,2})[:.\s](\d{1,2})$/.exec(trimmed)
  if (parts) {
    hours = parts[1]
    minutes = parts[2]
  } else {
    const digits = trimmed.replace(/\D/g, '')
    if (!digits || digits.length > 4 || digits.length !== trimmed.replace(/:/g, '').length) return null
    hours = digits.length <= 2 ? digits : digits.slice(0, digits.length - 2)
    minutes = digits.length <= 2 ? '0' : digits.slice(-2)
  }
  const hour = Number(hours)
  const minute = Number(minutes)
  if (hour > 23 || minute > 59) return null
  return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`
}
