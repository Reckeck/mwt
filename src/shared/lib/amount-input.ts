/** Thousands separator shown while typing: a narrow no-break space, never `,` or `.`. */
export const GROUP_SEPARATOR = ' '

/**
 * Most integer digits an amount may have (the Rust `Amount` input bound is 1e18).
 * Local until `src/shared/lib/limits.ts` exists (T46), which will own it.
 */
export const MAX_INTEGER_DIGITS = 18

export interface AmountNormalizeOptions {
  /** Most fraction digits kept; `undefined` keeps everything typed. */
  decimals?: number
  allowNegative?: boolean
  /** Decimal mark of the UI locale, used only for display. */
  decimalMark?: string
  /** Keep every integer digit of an over-long text (still flagged `overflow`) instead of cutting it to the cap. */
  keepOverflow?: boolean
}

export interface NormalizedAmount {
  /** What goes into the model: `-12345.67`, `12.` while typing, `''` when empty. */
  canonical: string
  /** What the field shows: `-12 345,67`. */
  display: string
  /** True when the text had more than `MAX_INTEGER_DIGITS` integer digits; `canonical`/`display` then hold the first ones only, unless `keepOverflow` is set. */
  overflow?: boolean
}

/** Insert the group separator every three digits from the right, in linear time. */
function groupDigits(digits: string): string {
  const head = digits.length % 3
  const groups: string[] = head ? [digits.slice(0, head)] : []
  for (let index = head; index < digits.length; index += 3) groups.push(digits.slice(index, index + 3))
  return groups.join(GROUP_SEPARATOR)
}

/** Integer digits of a canonical amount (`-12345.6` gives 5). */
export function integerDigitCount(canonical: string): number {
  return canonical.replace(/^-/, '').split('.')[0]?.length ?? 0
}

/** Fraction digits of a canonical amount (`12.50` gives 2, `12` gives 0). */
function fractionDigitCount(canonical: string): number {
  return canonical.split('.')[1]?.length ?? 0
}

/** True when `nextCanonical` is longer than the shown text: more integer digits, or as many and a longer fraction. */
export function growsPastCap(nextCanonical: string, shown: string, opts: AmountNormalizeOptions): boolean {
  const current = normalizeAmount(shown, { ...opts, keepOverflow: true }).canonical
  const nextInteger = integerDigitCount(nextCanonical)
  const currentInteger = integerDigitCount(current)
  if (nextInteger !== currentInteger) return nextInteger > currentInteger
  return fractionDigitCount(nextCanonical) > fractionDigitCount(current)
}

/**
 * Read whatever was typed or pasted into a plain decimal.
 *
 * Both `,` and `.` count as the decimal mark. When both appear (`1,234.50`,
 * `1.234,50`) the last one is the decimal mark and the others are grouping;
 * one mark repeated (`1,234,567`) is grouping; a single mark is decimal.
 * Spaces, currency signs and letters are dropped.
 */
export function normalizeAmount(text: string, opts: AmountNormalizeOptions = {}): NormalizedAmount {
  const { decimals, allowNegative = false, decimalMark = '.', keepOverflow = false } = opts
  const negative = allowNegative && /^\s*[-−]/.test(text)
  const cleaned = text.replace(/[^\d.,]/g, '')
  const marks = cleaned.replace(/\d/g, '')
  let intPart = cleaned
  let fracPart: string | null = null
  if (marks.length > 0) {
    const distinct = new Set(marks)
    const lastIndex = Math.max(cleaned.lastIndexOf(','), cleaned.lastIndexOf('.'))
    const isGrouping = distinct.size === 1 && marks.length > 1
    if (!isGrouping) {
      intPart = cleaned.slice(0, lastIndex)
      fracPart = cleaned.slice(lastIndex + 1)
    }
    intPart = intPart.replace(/[.,]/g, '')
    if (fracPart !== null) fracPart = fracPart.replace(/[.,]/g, '')
  }
  intPart = intPart.replace(/^0+(?=\d)/, '')
  if (fracPart !== null && decimals !== undefined) {
    fracPart = decimals === 0 ? null : fracPart.slice(0, decimals)
  }
  if (!intPart && fracPart === null) {
    return negative ? { canonical: '-', display: '-' } : { canonical: '', display: '' }
  }
  const overflow = intPart.length > MAX_INTEGER_DIGITS
  if (overflow && !keepOverflow) {
    // Cut before grouping: a pasted 5000-digit string stays cheap.
    const kept = intPart.slice(0, MAX_INTEGER_DIGITS)
    const signKept = negative ? '-' : ''
    return { canonical: `${signKept}${kept}`, display: `${signKept}${groupDigits(kept)}`, overflow: true }
  }
  const intDigits = intPart || '0'
  const sign = negative ? '-' : ''
  const canonical = `${sign}${intDigits}${fracPart !== null ? `.${fracPart}` : ''}`
  const display = `${sign}${groupDigits(intDigits)}${fracPart !== null ? `${decimalMark}${fracPart}` : ''}`
  return overflow ? { canonical, display, overflow } : { canonical, display }
}

/** Characters that carry meaning (digits, the decimal mark, the sign) before `caret`. */
export function significantBefore(text: string, caret: number): number {
  return text.slice(0, caret).replace(/[^\d.,\-−]/g, '').length
}

/** Caret position in `display` right after its `count`-th significant character. */
export function caretAfterSignificant(display: string, count: number): number {
  if (count <= 0) return 0
  let seen = 0
  for (let index = 0; index < display.length; index += 1) {
    if (/[\d.,-]/.test(display[index] ?? '')) seen += 1
    if (seen === count) return index + 1
  }
  return display.length
}
