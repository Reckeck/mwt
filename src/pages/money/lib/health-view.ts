import { dateFormat, formatPct, formatQuantity } from '@/shared/lib/format'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'

type Translate = (key: string, params?: Record<string, unknown>) => string
export type HealthTone = 'success' | 'warning' | 'destructive' | 'muted'

/** One figure of the checkup (Money 3 · 16): a label, the value, one line under it. */
export interface HealthFigure {
  key: 'fund' | 'debts' | 'savings'
  value: SumPart[]
  tone: HealthTone
  hint: SumPart[]
}

const text = (value: string): SumPart[] => [{ text: value }]

const DTI_ELEVATED = 30
const DTI_HIGH = 50

/** Whole percents without «,0», a fraction with one place. */
const pct = (value: string | null) => formatPct(value, { decimals: Number.isInteger(Number(value)) ? 0 : 1 })

/** `October`: the month as it reads after `of income`, genitive where the language has one. */
function monthOf(key: string): string {
  const date = new Date(`${key.slice(0, 7)}-01T12:00`)
  const parts = dateFormat({ day: 'numeric', month: 'long' }).formatToParts(date)
  return parts.find((part) => part.type === 'month')?.value ?? key
}

/** The month the savings card shows: the last finished one with income. */
export function lastSavings(report: HealthReport): SavingsMonth | null {
  return [...report.savings].reverse().find((month) => month.saved_pct !== null) ?? null
}

/** The sentence over the cards: everything fine, or the worst thing to fix first. */
export function healthPhrase(report: HealthReport, t: Translate): string {
  const issues = report.reasons.filter((reason) => reason.level !== 'green')
  if (report.color === 'no_data' || issues.some((reason) => reason.code === 'no_history'))
    return t('money.health.noData')
  const first = issues[0]
  if (!first) return t('money.health.ok')
  const what = t(`money.health.short.${first.code}`, {
    months: formatQuantity(first.pct, 1),
    target: report.emergency.fund.target_months,
    pct: pct(first.pct),
    plan: pct(report.savings_target_pct),
  })
  return issues.length === 1 ? t('money.health.one', { what }) : t('money.health.many', { count: issues.length, what })
}

function fundFigure(fund: EmergencyStatus, t: Translate): HealthFigure {
  const target = fund.fund.target_months
  if (!fund.stores.length)
    return {
      key: 'fund',
      value: text(t('money.health.none')),
      tone: 'destructive',
      hint: text(t('money.health.fundUnset')),
    }
  if (fund.months_covered === null)
    return { key: 'fund', value: text('—'), tone: 'muted', hint: text(t('health.fund.noSpending')) }
  const covered = Number(fund.months_covered)
  const { mark, parts } = composeSums()
  const money = (amount: string): string => mark({ amount, currency: fund.currency, decimals: 0 })
  const hint = fund.target_amount
    ? parts(t('money.health.fundHint', { target, total: money(fund.total), goal: money(fund.target_amount) }))
    : text(t('money.health.fundHintPlain', { target }))
  const tone = covered < 1 ? 'destructive' : covered < target ? 'warning' : 'success'
  return {
    key: 'fund',
    value: text(t('health.fund.level', { months: formatQuantity(fund.months_covered, 1) })),
    tone,
    hint,
  }
}

function debtsFigure(debts: DebtLoad, t: Translate): HealthFigure {
  if (!debts.loans.length)
    return { key: 'debts', value: text(t('money.health.none')), tone: 'success', hint: text(t('money.health.noLoans')) }
  const dti = debts.dti_pct === null ? null : Number(debts.dti_pct)
  const tone = dti === null || dti < DTI_ELEVATED ? 'muted' : dti < DTI_HIGH ? 'warning' : 'destructive'
  const { mark, parts } = composeSums()
  const payments = mark({ amount: debts.payments, currency: debts.currency, decimals: 0 })
  const hint =
    debts.dti_pct === null
      ? parts(t('money.health.paymentsPlain', { payments }))
      : parts(t('money.health.payments', { payments, pct: pct(debts.dti_pct) }))
  return { key: 'debts', value: [{ amount: debts.owed, currency: debts.currency, decimals: 0 }], tone, hint }
}

function savingsFigure(report: HealthReport, t: Translate): HealthFigure {
  const month = lastSavings(report)
  if (!month) return { key: 'savings', value: text('—'), tone: 'muted', hint: text(t('money.health.savingsNone')) }
  const saved = Number(month.saved_pct)
  // The «below plan» reason is about the average of the last months: the card says so too, not «all fine».
  const below = report.reasons.some((reason) => reason.code === 'savings_below_plan' && reason.level !== 'green')
  const tone = saved < 0 ? 'destructive' : below || saved < Number(report.savings_target_pct) ? 'warning' : 'success'
  return {
    key: 'savings',
    value: text(pct(month.saved_pct)),
    tone,
    hint: text(
      below && report.savings_avg_pct !== null
        ? t('money.health.savingsHintAvg', { month: monthOf(month.month), pct: pct(report.savings_avg_pct) })
        : t('money.health.savingsHint', { month: monthOf(month.month) }),
    ),
  }
}

/** Fund, debts and savings rate, in the order of the canvas. */
export function healthFigures(report: HealthReport, t: Translate): HealthFigure[] {
  return [fundFigure(report.emergency, t), debtsFigure(report.debts, t), savingsFigure(report, t)]
}

/** The fund can still get better: not set up or short of its months. */
export function fundCanImprove(fund: EmergencyStatus): boolean {
  return !fund.stores.length || Number(fund.months_covered ?? 0) < fund.fund.target_months
}
