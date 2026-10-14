export interface ResultRow {
  key: string
  label: string
  value: string
  emphasis?: boolean
  tone?: 'positive' | 'negative'
}

/** The lines under the calculator: what goes in and out, or the position's outcome. */
export function resultRows(
  calc: CalcResult,
  mode: CalcMode,
  baseCurrency: string,
  toSymbol: string | undefined,
  t: (key: string) => string,
): ResultRow[] {
  const money = (value: string) => `${value} ${baseCurrency.toUpperCase()}`
  if (mode === 'converter') {
    return [
      { key: 'gross_in', label: t('calc.grossIn'), value: money(calc.gross_in) },
      { key: 'fee', label: t('calc.fee'), value: money(calc.fee) },
      { key: 'gross_out', label: t('calc.grossOut'), value: money(calc.gross_out) },
      {
        key: 'net_out',
        label: t('calc.netOut'),
        value: `${calc.net_out} ${toSymbol?.toUpperCase() ?? ''}`.trim(),
        emphasis: true,
      },
    ]
  }
  const tone = Number(calc.pnl) < 0 ? 'negative' : 'positive'
  return [
    { key: 'spent', label: t('calc.spent'), value: money(calc.spent) },
    { key: 'exit_value', label: t('calc.exitValue'), value: calc.exit_value === '0' ? '—' : money(calc.exit_value) },
    { key: 'pnl', label: t('calc.pnl'), value: money(calc.pnl), tone },
    { key: 'roi_pct', label: t('calc.roi'), value: calc.roi_pct === '0' ? '—' : `${calc.roi_pct}%`, tone },
    {
      key: 'risk',
      label: t('calc.risk'),
      value: calc.risk === '0' ? '—' : `${money(calc.risk)} (${calc.risk_pct}%)`,
      tone: 'negative',
    },
  ]
}
