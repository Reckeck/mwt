import type { TransactionFormState } from './form-state'

/** What a calculator plan hands over: the same fields the plan calculator emits. */
export interface PlanFields {
  from: AssetRef | null
  to: AssetRef | null
  amount: string
  quantity?: string
  price: string
}

/**
 * Puts a calculator plan into a new form. A plan with both sides is an exchange (what was given,
 * what it buys): trades are read by the crypto module, nobody types them. A plan with only a
 * source is a top-up of it. Returns false when the wallet offers neither.
 */
export function applyPlan(form: TransactionFormState, plan: PlanFields, offered: readonly TxKind[]): boolean {
  if (plan.from && plan.to && offered.includes('convert')) {
    form.kind = 'convert'
    form.fromAsset = plan.from
    form.fromAmount = plan.amount
    form.toAsset = plan.to
    if (plan.quantity) form.toAmount = plan.quantity
  } else if (plan.from) {
    form.kind = offered.includes('topup') ? 'topup' : form.kind
    form.asset = plan.from
    form.amount = plan.amount
  } else return false
  if (plan.price) {
    form.price = plan.price
    form.priceCurrency = 'usd'
  }
  return true
}
