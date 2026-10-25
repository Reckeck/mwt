import type { SumPart } from '@/shared/lib/money-parts'

/** One row of the journal, ready to show. */
export interface JournalRow {
  entry: JournalEntry
  when: string
  did: SumPart[]
  status: string
  tone: string
}
