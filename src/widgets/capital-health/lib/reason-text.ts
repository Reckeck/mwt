import { formatPct, formatQuantity } from '@/shared/lib/format'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'

type Translate = (key: string, params?: Record<string, unknown>) => string

export interface ReasonText {
  /** What is wrong, in one sentence. */
  what: SumPart[]
  /** The first step to fix it. */
  step: SumPart[]
}

/** Whole percents without «,0»; a fraction keeps one place. */
export const percent = (value: string | null) => formatPct(value, { decimals: Number.isInteger(Number(value)) ? 0 : 1 })

/** A reason of the checkup and its first step: `health.reason.<code>` / `health.step.<code>`. */
export function reasonText(reason: HealthReason, currency: string, t: Translate): ReasonText {
  const { mark, parts } = composeSums()
  const params = {
    amount: mark({ amount: reason.amount, currency, decimals: 0 }),
    pct: percent(reason.pct),
    months: formatQuantity(reason.pct, 1),
    base: reason.code === 'lifestyle_creep' && reason.subject ? t(`insight.creepBase.${reason.subject}`) : '',
  }
  // A credit card is costly by its debt alone: no rate to name.
  const what =
    reason.code === 'costly_loan' && reason.pct === null ? 'health.reason.costly_card' : `health.reason.${reason.code}`
  // Without the monthly gap the step names no sum.
  const step =
    reason.code === 'savings_below_plan' && reason.amount === null
      ? 'health.step.savings_below_plan_plain'
      : `health.step.${reason.code}`
  return { what: parts(t(what, params)), step: parts(t(step, params)) }
}

/** The loan the reason is about, for its link; only loan reasons carry a store. */
export function reasonStore(reason: HealthReason): string | null {
  return reason.code === 'costly_loan' || reason.code === 'loan_without_rate' ? reason.subject : null
}
