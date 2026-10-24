/** One row of the journal, ready to show. */
export interface JournalRow {
  entry: JournalEntry
  when: string
  did: string
  status: string
  tone: string
}
