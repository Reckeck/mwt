export interface TransactionsListProps {
  transactions: Transaction[]
  /** Losses by transaction id, for the loss line of a row. */
  losses?: Record<string, TxLoss> | null
  /** Base units per USD and the base code, for the day totals. */
  fxPerUsd?: string | null
  baseCurrency?: string
  /** How many operations match in all: `Show more` asks the page for the next ones while fewer are loaded. */
  total: number
  /** The next page is on its way. */
  loadingMore?: boolean
}
