import { formatMoney, formatPct } from '@/shared/lib/format'

import type { CalcForm } from '../model/calc-form'

type Translate = (key: string, params?: Record<string, unknown>) => string

const numeric = (value: string) => Number(value.trim().replace(',', '.')) || 0
/** As many places as typed, up to two: «4 %», «3,74 %». */
const pct = (value: string) =>
  formatPct(numeric(value), { decimals: Math.min(2, (value.trim().split(/[.,]/)[1] ?? '').length) })

/**
 * The folded row's second line: what the rare settings hold now, so the user
 * sees what is counted without opening it — «инфляция 3,74 % · налога нет · снятий нет».
 */
export function advancedSummary(form: CalcForm, t: Translate): string {
  const parts: string[] = []
  if (numeric(form.indexation) > 0) parts.push(t('invest.summary.indexation', { pct: pct(form.indexation) }))
  parts.push(t('invest.summary.inflation', { pct: pct(form.inflation) }))
  parts.push(numeric(form.taxPct) > 0 ? t('invest.summary.tax', { pct: pct(form.taxPct) }) : t('invest.summary.noTax'))
  parts.push(
    form.withdrawal.on && numeric(form.withdrawal.amount) > 0
      ? t('invest.summary.withdrawal', {
          amount: formatMoney(numeric(form.withdrawal.amount), { currency: form.currency, decimals: 0 }),
          freq: t(`invest.freqShort.${form.withdrawal.freq}`),
        })
      : t('invest.summary.noWithdrawal'),
  )
  return parts.join(' · ')
}
