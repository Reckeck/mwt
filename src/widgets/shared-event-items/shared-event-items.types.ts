import type { PlanFact } from '@/entities/shared-event'

export interface SharedEventItemsProps {
  eventId: string
}

/** One row of the table: the item, its figures against the plan, and who bought it. */
export interface ItemRow {
  item: EventItem
  view: ItemView
  fact: PlanFact
  /** «по штукам · Петя 3, Вася 5», «только Петя, Вася»; empty when everybody shares. */
  caption: string
  /** «купил я», «купили Вася и я»; empty when nothing was bought. */
  boughtBy: string
  /** «8 из 6 шт · +600 ₽» as plain text: the accessible name of the status cell. */
  summary: string
}
