/** Thousands separator shown while typing: a narrow no-break space, never `,` or `.`. */
export const GROUP_SEPARATOR = ' '

export interface AmountNormalizeOptions {
  /** Most fraction digits kept; `undefined` keeps everything typed. */
  decimals?: number
  allowNegative?: boolean
  /** Decimal mark of the UI locale, used only for display. */
  decimalMark?: string
}

export interface NormalizedAmount {
  /** What goes into the model: `-12345.67`, `12.` while typing, `''` when empty. */
  canonical: string
  /** What the field shows: `-12 345,67`. */
  display: string
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
  const { decimals, allowNegative = false, decimalMark = '.' } = opts
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
  const intDigits = intPart || '0'
  const sign = negative ? '-' : ''
  const canonical = `${sign}${intDigits}${fracPart !== null ? `.${fracPart}` : ''}`
  const grouped = intDigits.replace(/\B(?=(\d{3})+(?!\d))/g, GROUP_SEPARATOR)
  const display = `${sign}${grouped}${fracPart !== null ? `${decimalMark}${fracPart}` : ''}`
  return { canonical, display }
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
