/** A stored operation back into the form, for editing or as a template. */

import { toRaw } from 'vue'
import { body, txKind } from '@/shared/lib/transactions'
import { toLocalInput } from '@/shared/lib/datetime'
import { stampToPrice } from './form-helpers'
import { emptyForm } from './form-state'
import type { TransactionFormState } from './form-state'

/** Load a stored transaction into the form for editing. */
export function formFromTransaction(stored: Transaction, storeId: string): TransactionFormState {
  // A copy: the form edits nested values (assets, accounts) that must not
  // leak back into the cached transaction it came from.
  const tx = structuredClone(toRaw(stored))
  const kind = txKind(tx)
  const form = emptyForm(storeId, kind)
  const meta = (tx as unknown as Record<TxKind, { meta: TxMeta; fee: Fee | null }>)[kind]
  form.categories = [...meta.meta.categories]
  form.comment = meta.meta.comment
  form.timestamp = toLocalInput(meta.meta.timestamp)
  form.originalTimestamp = meta.meta.timestamp
  form.contactId = meta.meta.contact_id ?? ''
  form.flowId = meta.meta.flow_id ?? ''
  if (meta.fee) {
    form.feeAmount = meta.fee.amount
    form.feeAsset = meta.fee.asset
  }
  switch (kind) {
    case 'topup': {
      const record = body(tx, 'topup')
      Object.assign(form, {
        storeId: record.store_id,
        asset: record.asset,
        amount: record.amount,
        counterpart: record.from,
        network: record.details.network ?? '',
        txHash: record.details.tx_hash ?? '',
        sourceStoreId: record.details.source_store_id ?? '',
        unitPrice: record.details.unit_price ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'payment': {
      const record = body(tx, 'payment')
      Object.assign(form, {
        storeId: record.store_id,
        asset: record.asset,
        amount: record.amount,
        counterpart: record.to,
        network: record.details.network ?? '',
        txHash: record.details.tx_hash ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      Object.assign(form, {
        asset: record.asset,
        amount: record.amount,
        from: record.from,
        to: record.to,
        debt: record.debt,
        network: record.network ?? '',
        txHash: record.tx_hash ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'convert': {
      const record = body(tx, 'convert')
      Object.assign(form, {
        from: record.from.account,
        to: record.to.account,
        fromAsset: record.from.asset,
        fromAmount: record.from.amount,
        toAsset: record.to.asset,
        toAmount: record.to.amount,
        rate: record.rate_override ?? '',
        ...stampToPrice(record.to.price),
      })
      break
    }
    case 'trade_spot': {
      const record = body(tx, 'trade_spot')
      Object.assign(form, {
        storeId: record.store_id,
        soldAsset: record.sold.asset,
        soldQty: record.sold.qty,
        boughtAsset: record.bought.asset,
        boughtQty: record.bought.qty,
        liquidity: record.liquidity ?? '',
        orderId: record.order_id ?? '',
        ...stampToPrice(record.bought.price),
      })
      break
    }
    case 'trade_futures': {
      const record = body(tx, 'trade_futures')
      const venue = record.contract.contract?.replace(/^perp:/, '') ?? 'bybit'
      Object.assign(form, {
        storeId: record.store_id,
        venue,
        symbol: record.contract.symbol,
        settle: record.settle,
        direction: record.direction,
        action: record.action,
        quantity: record.quantity,
        fillPrice: record.fill_price,
        leverage: record.leverage ?? '',
        closedPnl: record.closed_pnl ?? '',
        orderId: record.order_id ?? '',
        futuresCategory: record.category,
      })
      break
    }
    case 'defi': {
      const record = body(tx, 'defi')
      Object.assign(form, {
        storeId: record.store_id,
        defiAction: record.action,
        asset: record.asset,
        amount: record.amount,
        protocol: record.protocol ?? '',
        poolId: record.pool_id ?? '',
        network: record.network ?? '',
        txHash: record.tx_hash ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'asset_sell': {
      const record = body(tx, 'asset_sell')
      Object.assign(form, {
        storeId: record.store_id,
        asset: record.asset,
        sellPrice: record.unit_price,
        sellQty: record.quantity,
        currency: record.currency,
        proceedsTo: record.proceeds_to ?? '',
      })
      break
    }
  }
  return form
}
