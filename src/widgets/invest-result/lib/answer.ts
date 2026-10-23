import { dateFormat, numberFormat, pluralForm } from '@/shared/lib/format'

type Translate = (key: string, params?: Record<string, unknown>) => string

const MONTHS_IN_YEAR = 12

/** «10 лет», «9 лет 11 мес.», «7 мес.». */
export function termText(months: number, t: Translate, locale: string): string {
  const years = Math.floor(months / MONTHS_IN_YEAR)
  const rest = months % MONTHS_IN_YEAR
  const parts: string[] = []
  if (years) parts.push(t(`invest.yearsN.${pluralForm(years, locale)}`, { n: years }))
  if (rest || !years) parts.push(t('invest.monthsN', { n: rest }))
  return parts.join(' ')
}

/**
 * «окт 2029»: the month `month` of the plan (1-based) counted from its `start` («YYYY-MM»).
 * The plan's end — «к окт 2029» — is the month after its last: `term_months + 1`.
 */
export function planMonth(start: string, month: number): string {
  const [year, first] = start.split('-').map(Number)
  const date = new Date(year ?? 1970, (first ?? 1) - 1 + Math.max(0, month - 1), 1)
  const parts = dateFormat({ month: 'short', year: 'numeric' }).formatToParts(date)
  const pick = (type: string) => parts.find((part) => part.type === type)?.value ?? ''
  return `${pick('month').replace(/\.$/, '')} ${pick('year')}`
}

/**
 * «100 000 + 10 000 × 120»: how «Вложено» adds up, for a plain plan only —
 * one monthly stage from the start, no indexation.
 */
export function investedFormula(input: InvestInput): string | null {
  const [stage, ...more] = input.stages
  if (!stage || more.length || stage.freq !== 'monthly' || stage.from_month !== 1 || Number(input.indexation_pct) !== 0)
    return null
  const count = input.term_months
  if (!count || !(Number(stage.amount) > 0)) return null
  const number = numberFormat({ maximumFractionDigits: 2 })
  const tail = `${number.format(Number(stage.amount))} × ${count}`
  return Number(input.initial) > 0 ? `${number.format(Number(input.initial))} + ${tail}` : tail
}
