export interface RevalueRuleDialogProps {
  open?: boolean
  storeId: string
  rule: RevalueRule | null
  /** The object's currency: a floor in money is in it. */
  currency: string
}
