import type { AmountExpression } from '@/shared/ui/amount-input'
import { noteLine, withExpressionNote } from '@/shared/lib/amount-expr'

import type { TxFormState } from './use-tx-form-state'

/**
 * An expression typed into an amount field leaves `1200+350*2 = 1900` at the
 * end of the comment. One line per field: a new expression in the same field
 * replaces its line; the text the user wrote by hand stays as it was.
 */
export function useTxCalcNote(state: TxFormState) {
  const { form } = state
  const lines = new Map<string, string>()

  function noteExpression(field: string, { expr, result }: AmountExpression) {
    const line = noteLine(expr, result)
    form.comment = withExpressionNote(form.comment, lines.get(field) ?? null, line)
    lines.set(field, line)
  }

  return { noteExpression }
}
