export interface PendingImportsProps {
  /** Exchange wallet whose records are shown and synced; empty = every exchange (no sync button). */
  storeId?: string
  /** Show at most this many rows (the dashboard widget); 0 = all. */
  limit?: number
  /** Without the title and the hint, for a widget that has its own. */
  compact?: boolean
}
