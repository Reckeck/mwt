import type { ContactNet } from '@/entities/contact'

/** The chips above the table: who is listed. Archived contacts are only in `archive`. */
export type ContactScope = 'all' | 'debt' | 'quick' | 'archive'

/** The summary tiles filter by direction: who owes me, whom I owe. */
export type ContactDirection = 'receivable' | 'payable'

export interface ContactFilter {
  search: string
  scope: ContactScope
  direction: ContactDirection | null
  /** A contact kind, or empty for any. */
  kind: ContactKind | ''
}

/** One table row: the contact with what the table shows about it, worked out once. */
export interface ContactRow {
  id: string
  contact: Contact
  /** Debts per asset, the largest (by absolute value) first. */
  nets: ContactNet[]
  /** Net debt in the base currency, signed (positive: they owe me); the sort key of the debt column. */
  debtBase: number
  lastAt: string | null
  lastAmount: Amount | null
  lastAsset: string | null
  txCount: number
  accountCount: number
}

export interface ContactTotals {
  receivable: number
  payable: number
  receivableCount: number
  payableCount: number
  /** Some debt had no rate, so the base sums leave it out. */
  missingRate: boolean
}
