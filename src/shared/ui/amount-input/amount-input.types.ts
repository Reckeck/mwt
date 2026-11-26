export interface AmountInputProps {
  /** Most fraction digits accepted; unlimited when omitted (crypto quantities). */
  decimals?: number
  allowNegative?: boolean
  placeholder?: string
  disabled?: boolean
  id?: string
  /** `lg` — the one big sum of a form (limit, goal): taller, bigger digits. `hero` — the screen's amount at display size, centred with its currency (quick add on a phone).
   * `box` — the same amount in a bold-bordered box, number left and currency right (quick add on a desktop).
   * `inline` — no frame of its own, right-aligned: one side of an exchange box. */
  size?: 'default' | 'lg' | 'hero' | 'box' | 'inline'
  /** The «calculator» key on a phone; off for fields where a sum never makes sense. */
  calculator?: boolean
}

/** What the field reports when a typed expression became the amount: `1200+350*2` -> `1900`. */
export interface AmountExpression {
  expr: string
  result: string
}

/** What the hint under the field shows for the expression being typed. */
export type AmountExprOutcome =
  | { kind: 'result'; text: string }
  | { kind: 'error'; code: 'divByZero' | 'syntax' | 'negative' | 'tooLong' }
