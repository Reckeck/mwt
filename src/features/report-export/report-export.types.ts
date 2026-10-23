export interface ReportExportButtonProps {
  /** The reports on screen: one exports at once, several open a menu. */
  kinds: ReportKind[]
  filter: ReportFilter
}

export interface TransactionsExportButtonProps {
  /** The list's filter: what «операции по фильтру» writes. */
  filter: TxFilter
}
