import { evaluateAmountExpr, hasOperator } from '@/shared/lib/amount-expr'
import { normalizeAmount } from '@/shared/lib/amount-input'

/** What the on-screen keypad can press. */
export type PadKey =
  | { kind: 'digit'; value: string }
  | { kind: 'operator'; value: '+' | '-' | '*' | '/' }
  | { kind: 'decimal' }
  | { kind: 'backspace' }

const OPERATORS = /[+\-*/]$/
/** The number being typed: everything after the last operator. */
const LAST_NUMBER = /[^+\-*/]*$/

/**
 * The text the keypad builds, with `.` as the decimal mark. A press that would make nonsense
 * (a second mark in one number, an operator first, more integer digits than the cap) is ignored.
 */
export function applyPadKey(typed: string, key: PadKey): string {
  switch (key.kind) {
    case 'backspace':
      return typed.slice(0, -1)
    case 'decimal': {
      const current = LAST_NUMBER.exec(typed)?.[0] ?? ''
      if (current.includes('.')) return typed
      return `${typed}${current === '' ? '0' : ''}.`
    }
    case 'operator':
      if (!typed) return typed
      return OPERATORS.test(typed) ? `${typed.slice(0, -1)}${key.value}` : `${typed}${key.value}`
    case 'digit': {
      const next = `${typed}${key.value}`
      const current = LAST_NUMBER.exec(next)?.[0] ?? ''
      return normalizeAmount(current, { keepOverflow: true }).overflow ? typed : next
    }
  }
}

/** The canonical amount the typed text stands for: the number itself, or the result of an expression; `null` while it cannot be computed. */
export function padAmount(typed: string, decimals?: number): string | null {
  if (!hasOperator(typed)) return normalizeAmount(typed, { decimals }).canonical
  const result = evaluateAmountExpr(typed, { decimals })
  return result.ok ? normalizeAmount(result.value, { decimals }).canonical : null
}
