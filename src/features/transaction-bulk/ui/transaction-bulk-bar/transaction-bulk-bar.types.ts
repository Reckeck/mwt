import type { Selection } from '@/shared/lib/selection'

export interface TransactionBulkBarProps {
  selection: Selection
  /** The loaded rows: the picked ones are summed and checked for what applies. */
  transactions: readonly Transaction[]
}
