/**
 * The date a purchase starts with: today when it falls inside the event's days, else the first day
 * (shopping ahead of time). An event without dates takes today. All `YYYY-MM-DD`.
 */
export function defaultPurchaseDate(today: string, startsOn: string | null, endsOn: string | null): string {
  if (!startsOn) return today
  const last = endsOn ?? startsOn
  return today >= startsOn && today <= last ? today : startsOn
}
