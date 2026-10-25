const MONTH = /^\d{4}-\d{2}$/

/** A moment for a rate: a month `YYYY-MM` means its first day, anything else is passed as is (RFC 3339). */
export function moneyMoment(at: string | undefined): string {
  if (!at) return ''
  return MONTH.test(at) ? `${at}-01T00:00:00Z` : at
}
