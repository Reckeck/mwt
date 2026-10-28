import { type Ref, computed, ref } from 'vue'

import { compactExpr, evaluateAmountExpr, hasOperator } from '@/shared/lib/amount-expr'
import { type AmountNormalizeOptions, normalizeAmount } from '@/shared/lib/amount-input'

import type { AmountExprOutcome, AmountExpression } from './amount-input.types'

interface Context {
  display: Ref<string>
  /** Canonical model value; the model is touched only by a finished expression. */
  setModel: (canonical: string) => void
  options: () => AmountNormalizeOptions
  emitExpression: (payload: AmountExpression) => void
}

/**
 * The typed-expression side of an amount field. While the text holds an
 * operator the field shows it raw and the model keeps the last good number;
 * Enter/blur turns it into the result, Esc puts back what was there at focus.
 */
export function useAmountExpression(context: Context) {
  /** The raw text while it is an expression; `null` for a plain number. */
  const expr = ref<string | null>(null)
  const attempted = ref(false)
  let atFocus = ''

  const outcome = computed<AmountExprOutcome | null>(() => {
    if (expr.value === null) return null
    const { decimals, allowNegative, decimalMark } = context.options()
    const result = evaluateAmountExpr(expr.value, { decimals })
    if (!result.ok) {
      // Half-typed text («1200+») is not an error until the user tries to finish it.
      if (result.code === 'divByZero' || attempted.value) {
        return { kind: 'error', code: result.code === 'divByZero' ? 'divByZero' : 'syntax' }
      }
      return null
    }
    if (!allowNegative && result.value.startsWith('-')) return { kind: 'error', code: 'negative' }
    return { kind: 'result', text: normalizeAmount(result.value, { decimals, allowNegative, decimalMark }).display }
  })

  function onFocus() {
    atFocus = context.display.value
  }

  /** Typed text goes here first; `true` when it is an expression and was taken over. */
  function track(text: string): boolean {
    if (!hasOperator(text)) {
      expr.value = null
      attempted.value = false
      return false
    }
    expr.value = text
    context.display.value = text
    return true
  }

  /** Make the expression the amount. `false` when it cannot be computed (the field stays as it is). */
  function commit(text: string | null = expr.value): boolean {
    if (text === null) return true
    attempted.value = true
    expr.value = text
    const { decimals, allowNegative, decimalMark } = context.options()
    const result = evaluateAmountExpr(text, { decimals })
    if (!result.ok || (!allowNegative && result.value.startsWith('-'))) return false
    const next = normalizeAmount(result.value, { decimals, allowNegative, decimalMark })
    context.display.value = next.display
    context.setModel(next.canonical)
    context.emitExpression({ expr: compactExpr(text), result: next.canonical })
    expr.value = null
    attempted.value = false
    return true
  }

  /** Esc: back to what the field held when it got focus. */
  function cancel(): boolean {
    if (expr.value === null) return false
    context.display.value = atFocus
    expr.value = null
    attempted.value = false
    return true
  }

  /** Leaving the field: a computable expression is applied, a broken one is dropped. */
  function onBlur() {
    if (expr.value === null) return
    if (!commit()) cancel()
  }

  return { expr, attempted, outcome, onFocus, track, commit, cancel, onBlur }
}
