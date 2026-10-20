export interface AutomationAmountDialogProps {
  /** The operation booked approximately. */
  txId: string
  /** The automation's name, for the title. */
  name: string
  /** The approximate amount it was booked at: the field starts from it. */
  estimate?: string
  /** The asset's symbol, shown inside the field. */
  symbol?: string
}
