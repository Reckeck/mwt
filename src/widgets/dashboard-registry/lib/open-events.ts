import { decimalAdd } from '@/shared/lib/decimal'
import { sign } from '@/entities/shared-event'

/** What I am owed and owe in one currency across the open events. */
export interface CurrencyTotal {
  currency: string
  owedToMe: string
  iOwe: string
}

/** Events that still hang on money: not archived, not closed, and a sum between me and someone. */
export function openEvents(events: SharedEventSummary[]): SharedEventSummary[] {
  return events.filter(
    (event) => !event.archived && event.status !== 'closed' && (sign(event.owed_to_me) > 0 || sign(event.i_owe) > 0),
  )
}

/** The sums per currency, the one with the most owed to me first. */
export function totalsByCurrency(events: SharedEventSummary[]): CurrencyTotal[] {
  const totals = new Map<string, CurrencyTotal>()
  for (const event of events) {
    const row = totals.get(event.currency) ?? { currency: event.currency, owedToMe: '0', iOwe: '0' }
    row.owedToMe = decimalAdd(row.owedToMe, event.owed_to_me) ?? row.owedToMe
    row.iOwe = decimalAdd(row.iOwe, event.i_owe) ?? row.iOwe
    totals.set(event.currency, row)
  }
  return [...totals.values()].sort((left, right) => Number(right.owedToMe) - Number(left.owedToMe))
}
