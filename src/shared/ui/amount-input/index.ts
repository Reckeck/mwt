/**
 * Export surface of the amount field.
 * - `AmountInput`: the number field (groups digits, takes `1200+350*2` expressions, caps integer
 *   digits at `MAX_INTEGER_DIGITS` on typing and paste; sizes `default | lg | hero`).
 * - `AmountCalculator`: the phone's calculator sheet an `AmountInput` opens.
 * - Types: `AmountInputProps`, `AmountExpression`, `AmountExprOutcome`.
 * The on-screen number pad is `AmountKeypad` in `@/shared/ui/amount-keypad`.
 */
export { default as AmountInput } from './AmountInput.vue'
export { default as AmountCalculator } from './AmountCalculator.vue'
export * from './amount-input.types'
