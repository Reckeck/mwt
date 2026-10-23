export interface TransactionDeleteProps {
  /** Id of the transaction (`TxMeta.id`). */
  id: string
  /** «Удалить» with its word, for a card's footer. */
  labelled?: boolean
}
