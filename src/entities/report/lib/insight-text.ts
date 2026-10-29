import { formatMonthKey, formatPct } from '@/shared/lib/format'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'

type Translate = (key: string, params?: Record<string, unknown>) => string

export interface InsightTextContext {
  t: Translate
  /** Lowercase code the amounts are in. */
  currency: string
  locale: string
  /** Turns a category key into its name. */
  category: (key: string) => string
}

/** `March 2026` from `YYYY-MM`. */
export function monthName(key: string, locale: string): string {
  return formatMonthKey(key, { month: 'long', year: 'numeric' }, locale)
}

/** Which way a fact leans, for its icon and colour next to the words. */
export type InsightTone = 'good' | 'bad' | 'neutral'

const GOOD: InsightCode[] = ['saved', 'spent_down', 'wealth_up', 'profit', 'best_month', 'store_grew_most']
const BAD: InsightCode[] = [
  'overspent',
  'no_income',
  'spent_up',
  'below_plan',
  'wealth_down',
  'loss',
  'owes',
  'lifestyle_creep',
]

export function insightTone(fact: Insight): InsightTone {
  if (fact.code === 'real_change') return Number(fact.pct) < 0 ? 'bad' : 'good'
  if (GOOD.includes(fact.code)) return 'good'
  return BAD.includes(fact.code) ? 'bad' : 'neutral'
}

/** Codes whose `pct` may be missing. */
const OPTIONAL_PCT: InsightCode[] = ['wealth_up', 'wealth_down', 'top_category', 'conversion_losses']

/** Whole percents without «,0»; a fraction keeps one place. */
const percent = (value: string | null, signed = false) =>
  formatPct(value, { decimals: Number.isInteger(Number(value)) ? 0 : 1, signed })

/** One fact as a sentence of the UI language: `insight.<code>` with its numbers. */
export function insightText(fact: Insight, ctx: InsightTextContext): SumPart[] {
  const subject = fact.subject ?? ''
  const named =
    fact.code === 'top_category' || fact.code === 'category_grew'
      ? ctx.category(subject)
      : fact.code === 'costliest_month' || fact.code === 'best_month'
        ? monthName(subject, ctx.locale)
        : fact.code === 'top_asset'
          ? subject.toUpperCase()
          : fact.code === 'lifestyle_creep'
            ? ctx.t(`insight.creepBase.${subject}`)
            : subject
  // A fact without its percent (nothing to compare with) has its own wording.
  const key =
    fact.pct === null && OPTIONAL_PCT.includes(fact.code) ? `insight.${fact.code}_abs` : `insight.${fact.code}`
  const { mark, parts } = composeSums()
  return parts(
    ctx.t(key, {
      amount: mark({ amount: fact.amount, currency: ctx.currency, decimals: 0 }),
      pct: percent(fact.pct),
      // `real_change` carries the nominal percent in `amount`.
      nominal: percent(fact.amount, true),
      real: percent(fact.pct, true),
      subject: named,
    }),
  )
}
