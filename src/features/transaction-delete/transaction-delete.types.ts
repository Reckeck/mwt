export interface TransactionDeleteProps {
  /** Id of the transaction (`TxMeta.id`). */
  id: string
  /** `Delete` with its word, for a card's footer. */
  labelled?: boolean
}
