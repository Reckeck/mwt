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
