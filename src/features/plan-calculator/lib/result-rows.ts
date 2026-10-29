import { USDT } from '@/entities/asset'
import { formatPct, formatQuantity } from '@/shared/lib/format'
import type { SumPart } from '@/shared/lib/money-parts'

export interface ResultRow {
  key: string
  label: string
  /** Words and sums: the template draws a sum as `<Money>`. */
  value: SumPart[]
  tone?: 'positive' | 'negative'
}

const isZero = (value: string) => Number(value) === 0

/** The lines under the calculator: what goes in and out, or the position's outcome in one block. */
export function resultRows(
  calc: CalcResult,
  mode: CalcMode,
  baseCurrency: string,
  toSymbol: string | undefined,
  t: (key: string) => string,
): ResultRow[] {
  if (mode === 'converter') {
    const money = (amount: string, currency = baseCurrency): SumPart[] => [{ amount, currency }]
    return [
      { key: 'gross_in', label: t('calc.grossIn'), value: money(calc.gross_in) },
      { key: 'fee', label: t('calc.fee'), value: money(calc.fee) },
      { key: 'gross_out', label: t('calc.grossOut'), value: money(calc.gross_out) },
      { key: 'net_out', label: t('calc.netOut'), value: money(calc.net_out, toSymbol) },
    ]
  }
  // `Exit at target · Profit · Risk to stop · Risk / reward` (Money 4 · 23), all in the quote.
  const quote = (amount: string, signed = false): SumPart => ({ amount, currency: USDT.symbol, decimals: 2, signed })
  const pct = (value: string) => formatPct(value, { signed: true, decimals: 1 })
  const rows: ResultRow[] = []
  if (!isZero(calc.exit_value)) {
    rows.push({ key: 'exit_value', label: t('calc.exitValue'), value: [quote(calc.exit_value)] })
    rows.push({
      key: 'pnl',
      label: t('calc.pnl'),
      value: [quote(calc.pnl, true), { text: ` · ${pct(calc.roi_pct)}` }],
      tone: Number(calc.pnl) < 0 ? 'negative' : 'positive',
    })
  }
  if (!isZero(calc.risk)) {
    rows.push({
      key: 'risk',
      label: t('calc.risk'),
      // A loss up to the stop, so the true minus sign («−52,03 USDT · −5,2 %»).
      value: [{ text: '\u2212' }, quote(calc.risk), { text: ` · \u2212${formatPct(calc.risk_pct, { decimals: 1 })}` }],
      tone: 'negative',
    })
  }
  if (!isZero(calc.reward_risk)) {
    rows.push({
      key: 'reward_risk',
      label: t('calc.rewardRisk'),
      value: [{ text: `1 : ${formatQuantity(calc.reward_risk, 1)}` }],
    })
  }
  return rows
}
