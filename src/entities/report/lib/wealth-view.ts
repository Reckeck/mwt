/** The groups capital is stacked by (Money 2 · 12), bottom to top. */
export type WealthGroup = 'property' | 'crypto' | 'bank' | 'owed' | 'debts'

/** The asset groups the chart stacks; what is owed and owing only shows in the table. */
export const STACKED: WealthGroup[] = ['property', 'crypto', 'bank']

/** One group's value at a point: debts come out negative. */
export function groupValue(point: NetworthPoint, group: WealthGroup): number {
  switch (group) {
    case 'property':
      return Number(point.non_financial)
    case 'crypto':
      return Number(point.crypto)
    case 'bank':
      return Number(point.financial) - Number(point.crypto)
    case 'owed':
      return Number(point.receivables)
    case 'debts':
      return -(Number(point.liabilities) + Number(point.payables))
  }
}

export interface WealthRow {
  group: WealthGroup
  value: number
  /** Share of everything owned, 0–100; `null` for debts. */
  share: number | null
}

/** «Из чего»: the groups at the last point, biggest first, debts last; empty groups left out. */
export function wealthRows(point: NetworthPoint | undefined): WealthRow[] {
  if (!point) return []
  const owned = (['property', 'crypto', 'bank', 'owed'] as const).map((group) => ({
    group,
    value: groupValue(point, group),
  }))
  const total = owned.reduce((sum, row) => sum + Math.max(0, row.value), 0)
  const rows: WealthRow[] = owned
    .filter((row) => row.value !== 0)
    .sort((left, right) => right.value - left.value)
    .map((row) => ({ ...row, share: total > 0 ? Math.round((Math.max(0, row.value) / total) * 100) : 0 }))
  const debts = groupValue(point, 'debts')
  if (debts !== 0) rows.push({ group: 'debts', value: debts, share: null })
  return rows
}

export interface WealthChange {
  /** Since the month before the last point. */
  month: number
  monthPct: number | null
  /** Since the first point. */
  period: number
  months: number
}

export function wealthChange(points: NetworthPoint[]): WealthChange | null {
  const last = points.at(-1)
  if (!last || points.length < 2) return null
  const net = Number(last.net)
  const before = Number(points.at(-2)?.net ?? 0)
  return {
    month: net - before,
    monthPct: before > 0 ? ((net - before) / before) * 100 : null,
    period: net - Number(points[0]?.net ?? 0),
    months: points.length - 1,
  }
}

/** The group that moved capital most over the last month: «Выросло в основном за счёт Крипты». */
export function wealthDriver(points: NetworthPoint[]): { group: WealthGroup; delta: number } | null {
  const last = points.at(-1)
  const before = points.at(-2)
  if (!last || !before) return null
  const moves = (['property', 'crypto', 'bank', 'owed', 'debts'] as const).map((group) => ({
    group,
    delta: groupValue(last, group) - groupValue(before, group),
  }))
  const top = moves.reduce((best, move) => (Math.abs(move.delta) > Math.abs(best.delta) ? move : best))
  return top.delta === 0 ? null : top
}
