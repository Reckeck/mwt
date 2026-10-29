import { decimalSub } from '@/shared/lib/decimal'

import type { TransactionFormState } from './form-state'

/**
 * An exchange of one currency for itself is a transfer between wallets: the
 * same wallets, date, comment and files go on as a `Transfer`; what the
 * receiving side got short of the given sum becomes the transfer's fee.
 */
export function convertAsTransfer(form: TransactionFormState): void {
  const short = decimalSub(form.fromAmount, form.toAmount)
  form.kind = 'transfer'
  form.asset = form.fromAsset
  form.amount = form.fromAmount
  if (short !== null && Number(short) > 0) {
    form.feeAmount = short
    form.feeAsset = null
  }
}
