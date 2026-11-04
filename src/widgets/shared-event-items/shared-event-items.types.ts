import type { PlanFact } from '@/entities/shared-event'

export interface SharedEventItemsProps {
  eventId: string
}

/** One row of the table: the item, its figures against the plan, and who bought it. */
export interface ItemRow {
  item: EventItem
  view: ItemView
  fact: PlanFact
  /** `by units · Pete 3, Vasya 5`, `only Pete, Vasya`; empty when everybody shares. */
  caption: string
  /** `I bought`, `Vasya and I bought`; empty when nothing was bought. */
  boughtBy: string
  /** `8 of 6 pcs · +600 ₽` as plain text: the accessible name of the status cell. */
  summary: string
  /** A purchase of the item has the automatic rate 1 in another currency than the event's. */
  needsRate: boolean
}
