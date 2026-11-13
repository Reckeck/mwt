/** The form as the backend input of its kind. */

import { common, details, feeInput, ident, text, viaComment, viaContact } from './form-helpers'
import type { TransactionFormState } from './form-state'

/** The asset `common.price_usd` prices: the one received, for two-asset kinds. */
export function pricedAsset(form: TransactionFormState): AssetRef | null {
  switch (form.kind) {
    case 'convert':
      return form.toAsset
    case 'trade_spot':
      return form.boughtAsset
    case 'trade_futures':
    case 'asset_sell':
      return null
    default:
      return form.asset
  }
}

/** Build the backend input, or `null` while a required asset is missing. */
export function formToInput(form: TransactionFormState): TransactionInput | null {
  const shared = { common: common(form), fee: feeInput(form) }
  switch (form.kind) {
    case 'topup':
      if (!form.asset) return null
      return {
        type: 'topup',
        ...shared,
        store_id: form.storeId,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        from: form.counterpart,
        details: details(form),
      }
    case 'payment':
      if (!form.asset) return null
      return {
        type: 'payment',
        ...shared,
        store_id: form.storeId,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        to: form.counterpart,
        details: details(form),
      }
    case 'transfer':
      if (!form.asset || !form.from || !form.to) return null
      return {
        type: 'transfer',
        ...shared,
        from: form.from,
        to: form.to,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        network: form.network || null,
        tx_hash: text(form.txHash),
        debt: form.debt,
      }
    case 'convert': {
      if (!form.fromAsset || !form.toAsset || !form.from || !form.to) return null
      const through = form.via !== null
      return {
        type: 'convert',
        ...shared,
        common: through
          ? {
              ...shared.common,
              contact_id: viaContact(form.via) ?? shared.common.contact_id,
              comment: viaComment(form.via, shared.common.comment),
            }
          : shared.common,
        from: { account: form.from, asset: ident(form.fromAsset), amount: form.fromAmount.trim() },
        to: { account: form.to, asset: ident(form.toAsset), amount: form.toAmount.trim() },
        rate_override: text(form.rate),
      }
    }
    case 'trade_spot':
      if (!form.soldAsset || !form.boughtAsset) return null
      return {
        type: 'trade_spot',
        ...shared,
        store_id: form.storeId,
        sold: { asset: ident(form.soldAsset), qty: form.soldQty.trim() },
        bought: { asset: ident(form.boughtAsset), qty: form.boughtQty.trim() },
        liquidity: form.liquidity || null,
        order_id: text(form.orderId),
      }
    case 'trade_futures':
      if (!form.settle) return null
      return {
        type: 'trade_futures',
        ...shared,
        store_id: form.storeId,
        venue: form.venue.trim() || 'bybit',
        symbol: form.symbol.trim().toUpperCase(),
        settle: ident(form.settle),
        direction: form.direction,
        action: form.action,
        quantity: form.quantity.trim(),
        fill_price: form.fillPrice.trim(),
        leverage: text(form.leverage),
        closed_pnl: form.action === 'close' ? text(form.closedPnl) : null,
        order_id: text(form.orderId),
        category: form.futuresCategory,
      }
    case 'defi':
      if (!form.asset) return null
      return {
        type: 'defi',
        ...shared,
        store_id: form.storeId,
        action: form.defiAction,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        protocol: text(form.protocol),
        pool_id: text(form.poolId),
        network: form.network || null,
        tx_hash: text(form.txHash),
      }
    case 'asset_sell':
      if (!form.asset || !form.currency) return null
      return {
        type: 'asset_sell',
        ...shared,
        store_id: form.storeId,
        asset: ident(form.asset),
        unit_price: form.sellPrice.trim(),
        quantity: form.sellQty.trim(),
        currency: ident(form.currency),
        proceeds_to: text(form.proceedsTo),
      }
    // A correction has its own screen (`AdjustmentDialog`).
    case 'adjustment':
      return null
  }
}
