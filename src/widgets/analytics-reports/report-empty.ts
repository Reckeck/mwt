const isZero = (value: string | null | undefined) => !value || Number(value) === 0

/**
 * A report with nothing to draw for the chosen period and wallets: the view
 * then says so instead of showing empty axes. Months the backend returns with
 * zeros (cash flow) or marks empty (year) count as nothing.
 */
export function reportIsEmpty(report: AnalyticsReport): boolean {
  if ('cashflow' in report) {
    return report.cashflow.rows.every((row) =>
      [row.income, row.fixed, row.daily, row.savings_moved, row.net].every(isZero),
    )
  }
  if ('year' in report) return report.year.rows.every((row) => row.empty)
  if ('categories' in report) return !report.categories.top.length && !report.categories.groups.length
  if ('networth' in report) return !report.networth.points.length
  if ('stores' in report) return !report.stores.rows.length
  if ('pnl' in report) return !report.pnl.rows.length
  if ('portfolio' in report) return !report.portfolio.points.length
  if ('inflation' in report) return !report.inflation.points.length
  return report.losses.rows.every((row) => isZero(row.fee) && isZero(row.slippage))
}
