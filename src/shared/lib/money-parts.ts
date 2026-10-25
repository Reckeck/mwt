import { formatMoney } from './format'

/**
 * A piece of a line that holds sums: words, or a sum a template draws as `<Money>`.
 * A `lib/` function returns these instead of a string, so the sum stays hoverable.
 */
export type SumPart =
  | { text: string }
  | {
      amount: string | number | null | undefined
      currency?: string
      decimals?: number
      signed?: boolean
      compact?: boolean
      bare?: boolean
      /** `false` for a number that is not a sum to convert (a rate). */
      convert?: boolean
      at?: string
    }

export type Sum = Exclude<SumPart, { text: string }>

const MARK = '\u0001'
const SPLIT = new RegExp(`${MARK}(\\d+)${MARK}`)

export const isSum = (part: SumPart): part is Sum => 'amount' in part

/**
 * The one way to word a line that holds sums. `mark(sum)` (or `mark(parts)`) gives a placeholder to
 * pass to `t(…)` or to put into a string; `parts(text)` cuts the finished text into words and sums
 * (`SumPart`s), so a view draws each sum as `<Money>` (through `<MoneyPhrase>`).
 */
export function composeSums() {
  const marked: (Sum | SumPart[])[] = []
  return {
    mark(sum: Sum | SumPart[]): string {
      marked.push(sum)
      return `${MARK}${marked.length - 1}${MARK}`
    },
    parts(text: string): SumPart[] {
      return text.split(SPLIT).flatMap((piece, index): SumPart[] => {
        if (index % 2 === 0) return piece ? [{ text: piece }] : []
        const found = marked[Number(piece)]
        if (!found) return []
        return Array.isArray(found) ? found : [found]
      })
    },
  }
}

/** The parts as one plain string, for an attribute that cannot hold a card (title, aria-label). */
export function partsText(parts: SumPart[]): string {
  return parts
    .map((part) => {
      if (!isSum(part)) return part.text
      // money: plain — title / aria-label text: an attribute cannot open a card
      return formatMoney(part.amount, {
        currency: part.currency,
        decimals: part.decimals,
        signed: part.signed,
        compact: part.compact,
      })
    })
    .join('')
}
