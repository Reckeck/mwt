/**
 * Exact arithmetic on the decimal strings `Amount` travels as. Display only —
 * the backend owns every rule — but a difference shown to the user must not
 * read `0.30000000000000004`.
 */

const DECIMAL = /^-?\d+(\.\d+)?$/

/** `raw` with a comma decimal mark accepted, or `null` when it is not a number. */
export function parseDecimal(raw: string): string | null {
  const text = raw.trim().replace(',', '.')
  return DECIMAL.test(text) ? text : null
}

function scaled(value: string, places: number): bigint {
  const negative = value.startsWith('-')
  const [whole, fraction = ''] = (negative ? value.slice(1) : value).split('.')
  const digits = BigInt(whole + fraction.padEnd(places, '0'))
  return negative ? -digits : digits
}

function places(value: string): number {
  return value.split('.')[1]?.length ?? 0
}

function unscaled(value: bigint, digits: number): string {
  const negative = value < 0n
  const text = (negative ? -value : value).toString().padStart(digits + 1, '0')
  const whole = digits ? text.slice(0, -digits) : text
  const fraction = digits ? text.slice(-digits).replace(/0+$/, '') : ''
  const out = fraction ? `${whole}.${fraction}` : whole
  return negative && out !== '0' ? `-${out}` : out
}

/** `left − right`, exactly; `null` when either side is not a number. */
export function decimalSub(left: string, right: string): string | null {
  const minuend = parseDecimal(left)
  const subtrahend = parseDecimal(right)
  if (minuend === null || subtrahend === null) return null
  const digits = Math.max(places(minuend), places(subtrahend))
  return unscaled(scaled(minuend, digits) - scaled(subtrahend, digits), digits)
}

/** `left + right`, exactly; `null` when either side is not a number. */
export function decimalAdd(left: string, right: string): string | null {
  const negated = parseDecimal(right)
  if (negated === null) return null
  return decimalSub(left, negated.startsWith('-') ? negated.slice(1) : `-${negated}`)
}

/** `left × right`, exactly; `null` when either side is not a number. */
export function decimalMul(left: string, right: string): string | null {
  const lhs = parseDecimal(left)
  const rhs = parseDecimal(right)
  if (lhs === null || rhs === null) return null
  return unscaled(scaled(lhs, places(lhs)) * scaled(rhs, places(rhs)), places(lhs) + places(rhs))
}

/** A computed number for an input: at most `digits` places, no trailing zeros; `''` when it is not a positive finite number. */
export function decimalInput(value: number, digits: number): string {
  return Number.isFinite(value) && value > 0 ? String(+value.toFixed(digits)) : ''
}

/** Sign of `left − right`; `null` when either side is not a number. */
export function decimalCmp(left: string, right: string): -1 | 0 | 1 | null {
  const lhs = parseDecimal(left)
  const rhs = parseDecimal(right)
  if (lhs === null || rhs === null) return null
  const digits = Math.max(places(lhs), places(rhs))
  const diff = scaled(lhs, digits) - scaled(rhs, digits)
  return diff === 0n ? 0 : diff < 0n ? -1 : 1
}

/**
 * `given × part ÷ total` rounded half away from zero to `digits` places;
 * `null` when an input is not a number or `total` is not positive.
 */
export function decimalShare(given: string, part: string, total: string, digits: number): string | null {
  const amount = parseDecimal(given)
  const portion = parseDecimal(part)
  const whole = parseDecimal(total)
  if (amount === null || portion === null || whole === null) return null
  const divisor = scaled(whole, places(whole))
  if (divisor <= 0n) return null
  const numerator =
    scaled(amount, places(amount)) * scaled(portion, places(portion)) * 10n ** BigInt(digits + places(whole))
  const denominator = divisor * 10n ** BigInt(places(amount) + places(portion))
  const magnitude = numerator < 0n ? -numerator : numerator
  const rounded = (2n * magnitude + denominator) / (2n * denominator)
  return unscaled(numerator < 0n ? -rounded : rounded, digits)
}
