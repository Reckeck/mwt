import { assetKey } from '@/shared/lib/asset-ref'
import { decimalSub, parseDecimal } from '@/shared/lib/decimal'

import type { AdjustmentRow } from '../store-adjust.types'

/**
 * The ledger's balances as rows; what the user typed (or the edited correction
 * held) stays put when the date moves the balances.
 */
export function mergeDraftRows(
  data: AdjustmentDraft,
  current: AdjustmentRow[],
  editing: AdjustmentTx | null,
): AdjustmentRow[] {
  const typed = new Map(current.map((row) => [assetKey(row.asset), row]))
  const next: AdjustmentRow[] = data.lines.map((line) => ({
    asset: line.asset,
    before: line.before,
    actual: typed.get(assetKey(line.asset))?.actual ?? '',
  }))
  const known = new Set(next.map((row) => assetKey(row.asset)))
  for (const row of typed.values()) {
    if (!known.has(assetKey(row.asset))) next.push({ ...row, before: '0' })
  }
  if (!current.length && editing) {
    for (const line of editing.lines) {
      const row = next.find((entry) => assetKey(entry.asset) === assetKey(line.asset))
      if (row) row.actual = line.after
      else next.push({ asset: line.asset, before: '0', actual: line.after })
    }
  }
  return next
}

/** Actual minus ledger, exactly; `null` while the field is empty or not a number. */
export const difference = (row: AdjustmentRow): string | null =>
  row.actual.trim() ? decimalSub(row.actual, row.before) : null

/** A typed balance that is not a number. */
export const badRow = (row: { actual: string }) => Boolean(row.actual.trim()) && parseDecimal(row.actual) === null

/** The correction's lines for the backend. */
export function adjustmentLines(rows: AdjustmentRow[]): AdjustmentLineInput[] {
  return rows.map((row) => ({
    asset: {
      symbol: row.asset.symbol,
      name: row.asset.name,
      coingecko_id: row.asset.coingecko_id,
      chain: row.asset.chain,
      contract: row.asset.contract,
    },
    actual: parseDecimal(row.actual) ?? row.actual,
  }))
}
