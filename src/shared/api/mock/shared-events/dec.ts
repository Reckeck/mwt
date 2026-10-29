/**
 * Money of the mock as exact integers: an amount is a `bigint` of 1e-8 units,
 * the way `Amount` is a decimal in Rust. No floats touch a sum.
 */

export const SCALE = 100_000_000n
/** One cent. */
export const CENT = 1_000_000n

/** `12.5`, `-3`, `4,5` to units; digits past the eighth place are dropped. */
export function dec(raw: string | null | undefined): bigint {
  const text = (raw ?? '').trim().replace(',', '.')
  const match = /^(-?)(\d*)(?:\.(\d*))?$/.exec(text)
  if (!match || (!match[2] && !match[3])) return 0n
  const [, sign, whole = '', fraction = ''] = match
  const units = BigInt(whole || '0') * SCALE + BigInt(fraction.slice(0, 8).padEnd(8, '0'))
  return sign ? -units : units
}

/** Units back to an amount string without needless zeros. */
export function fmt(value: bigint): string {
  const negative = value < 0n
  const abs = negative ? -value : value
  const whole = abs / SCALE
  const fraction = (abs % SCALE).toString().padStart(8, '0').replace(/0+$/, '')
  const text = fraction ? `${whole}.${fraction}` : `${whole}`
  return negative && abs !== 0n ? `-${text}` : text
}

export const abs = (value: bigint): bigint => (value < 0n ? -value : value)

/** Integer division rounded half away from zero (the divisor may be negative). */
export function divRound(numerator: bigint, divisor: bigint): bigint {
  const negative = numerator < 0n !== divisor < 0n
  const top = abs(numerator)
  const bottom = abs(divisor)
  const quotient = (top * 2n + bottom) / (bottom * 2n)
  return negative ? -quotient : quotient
}

/** `value` rounded half away from zero to a multiple of `step`. */
export const roundTo = (value: bigint, step: bigint): bigint => divRound(value, step) * step

export const round2 = (value: bigint): bigint => roundTo(value, CENT)

/** The exact product of the factors, rounded once to the cent. */
export function mulRound2(...factors: bigint[]): bigint {
  const product = factors.reduce((sum, factor) => sum * factor, 1n)
  return divRound(product, SCALE ** BigInt(factors.length - 1) * CENT) * CENT
}

export const sum = (values: bigint[]): bigint => values.reduce((total, value) => total + value, 0n)
