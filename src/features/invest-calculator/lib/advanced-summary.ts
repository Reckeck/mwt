import { formatPct } from '@/shared/lib/format'
import { composeSums, type Sum, type SumPart } from '@/shared/lib/money-parts'

import type { CalcForm } from '../model/calc-form'

type Translate = (key: string, params?: Record<string, unknown>) => string

const numeric = (value: string) => Number(value.trim().replace(',', '.')) || 0
/** As many places as typed, up to two: «4 %», «3,74 %». */
const pct = (value: string) =>
  formatPct(numeric(value), { decimals: Math.min(2, (value.trim().split(/[.,]/)[1] ?? '').length) })

/**
 * The folded row's second line: what the rare settings hold now, so the user
 * sees what is counted without opening it — `inflation 3.74 % · no tax · no withdrawals`.
 */
export function advancedSummary(form: CalcForm, t: Translate): SumPart[] {
  const parts: SumPart[][] = []
  const words = (text: string) => parts.push([{ text }])
  if (numeric(form.indexation) > 0) words(t('invest.summary.indexation', { pct: pct(form.indexation) }))
  words(t('invest.summary.inflation', { pct: pct(form.inflation) }))
  words(numeric(form.taxPct) > 0 ? t('invest.summary.tax', { pct: pct(form.taxPct) }) : t('invest.summary.noTax'))
  if (form.withdrawal.on && numeric(form.withdrawal.amount) > 0) {
    const sum: Sum = { amount: numeric(form.withdrawal.amount), currency: form.currency, decimals: 0 }
    const { mark, parts: phrase } = composeSums()
    parts.push(
      phrase(
        t('invest.summary.withdrawal', { amount: mark(sum), freq: t(`invest.freqShort.${form.withdrawal.freq}`) }),
      ),
    )
  } else words(t('invest.summary.noWithdrawal'))
  return parts.flatMap((part, at): SumPart[] => (at ? [{ text: ' · ' }, ...part] : part))
}
