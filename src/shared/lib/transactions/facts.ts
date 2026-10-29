/** Kind-specific facts for the details view: extra fields, the fee, the debt effect. */

import { formatMonthKey, formatQuantity } from '../format'

import { anyBody, body, txKind, txMeta } from './access'

/** Kind-specific facts worth showing under a transaction, as `[i18n key, value]`. */
export function txDetails(tx: Transaction): Array<[string, string]> {
  const out: Array<[string, string]> = []
  switch (txKind(tx)) {
    case 'topup': {
      const { details } = body(tx, 'topup')
      if (details.unit_price) out.push(['tx.field.unitPrice', details.unit_price])
      break
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      if (record.network) out.push(['tx.field.network', record.network])
      break
    }
    case 'convert': {
      const record = body(tx, 'convert')
      if (record.rate_override) out.push(['tx.field.rate', record.rate_override])
      break
    }
    case 'trade_spot': {
      const record = body(tx, 'trade_spot')
      if (record.liquidity) out.push(['tx.field.liquidity', record.liquidity])
      if (record.order_id) out.push(['tx.field.order', record.order_id])
      break
    }
    case 'trade_futures': {
      const record = body(tx, 'trade_futures')
      out.push(['tx.field.position', `${record.direction} · ${record.action}`])
      out.push(['tx.field.price', record.fill_price])
      if (record.leverage) out.push(['tx.field.leverage', `${record.leverage}×`])
      break
    }
    case 'defi': {
      const record = body(tx, 'defi')
      out.push(['tx.field.action', record.action])
      if (record.protocol) out.push(['tx.field.protocol', record.protocol])
      if (record.pool_id) out.push(['tx.field.pool', record.pool_id])
      break
    }
    case 'asset_sell': {
      const record = body(tx, 'asset_sell')
      out.push(['tx.field.unitPrice', `${record.unit_price} ${record.currency.symbol}`])
      break
    }
    case 'adjustment': {
      for (const line of body(tx, 'adjustment').lines) {
        out.push([
          'adjustment.line',
          `${formatQuantity(line.before)} → ${formatQuantity(line.after)} ${line.asset.symbol}`,
        ])
      }
      break
    }
    default:
      break
  }
  return out
}

/** The fee recorded on the transaction, with the asset it was paid in. */
export function txFee(tx: Transaction): { amount: string; asset: string } | null {
  const fee = anyBody(tx).fee
  if (!fee || Number(fee.amount) === 0) return null
  return { amount: fee.amount, asset: fee.asset.symbol }
}

/** Debt effect of a transfer, if it is one. */
export function txDebt(tx: Transaction): DebtEffect | null {
  return txKind(tx) === 'transfer' ? body(tx, 'transfer').debt : null
}

/**
 * The automation or workflow that booked it (`Auto` in the row, Paths 2 · 10);
 * `null` for one entered by hand or imported. An automation runs as a workflow
 * of the same id, so both origins name it the same way.
 */
export function txAutomation(tx: Transaction): { id: string; runId: string | null } | null {
  const origin = txMeta(tx).origin
  if (typeof origin !== 'object') return null
  if ('automation' in origin) return { id: origin.automation.automation_id, runId: null }
  if ('workflow' in origin) return { id: origin.workflow.workflow_id, runId: origin.workflow.run_id }
  return null
}

/** «YYYY-MM» of the month an income is for when that is not its date's own month; `null` otherwise. */
export function txForMonth(tx: Transaction): string | null {
  if (txKind(tx) !== 'topup') return null
  const month = body(tx, 'topup').details.for_month
  if (!month) return null
  return month === txMeta(tx).timestamp.slice(0, 7) ? null : month
}

/** The month an income is for as a name (`September`), with the year when it is not its date's year; `null` if none. */
export function txForMonthLabel(tx: Transaction): string | null {
  const month = txForMonth(tx)
  if (!month) return null
  const sameYear = month.slice(0, 4) === txMeta(tx).timestamp.slice(0, 4)
  return formatMonthKey(month, sameYear ? { month: 'long' } : { month: 'long', year: 'numeric' })
}
