export interface SplitRowsProps {
  /** `convert` or `topup`. */
  kind: TxKind
  /** An exchange through an intermediary: the money lands on your own accounts only. */
  via?: boolean
}
