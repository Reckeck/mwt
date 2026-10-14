/** Small pieces the input builders share: idents, the fee, the price stamp, P2P sides, common fields. */

import { keepOriginalMoment } from '@/shared/lib/datetime'
import type { TransactionFormState } from './form-state'

export const text = (raw: string): string | null => raw.trim() || null

export function ident(asset: AssetRef): AssetIdent {
  return {
    symbol: asset.symbol,
    name: asset.name,
    coingecko_id: asset.coingecko_id,
    chain: asset.chain,
    contract: asset.contract,
  }
}

/** The asset a fee defaults to: what the kind mainly moves. */
function mainAsset(form: TransactionFormState): AssetRef | null {
  switch (form.kind) {
    case 'convert':
      return form.fromAsset
    case 'trade_spot':
      return form.boughtAsset
    case 'trade_futures':
      return form.settle
    case 'asset_sell':
      return form.currency
    default:
      return form.asset
  }
}

export function feeInput(form: TransactionFormState): FeeInput | null {
  const asset = form.feeAsset ?? mainAsset(form)
  if (!form.feeAmount.trim() || Number(form.feeAmount) === 0 || !asset) return null
  return { asset: ident(asset), amount: form.feeAmount.trim() }
}

/** A stored stamp back into the form: the base-currency price when it was typed so, else USD. */
export function stampToPrice(stamp: PriceStamp): Pick<TransactionFormState, 'price' | 'priceCurrency'> {
  if (stamp.price_base) return { price: stamp.price_base, priceCurrency: '' }
  return { price: stamp.price_usd ?? '', priceCurrency: stamp.price_usd ? 'usd' : '' }
}

/** P2P through a contact: the operation is with them. */
export function viaContact(via: AccountRef | null): string | null {
  return via && 'contact' in via ? via.contact.contact_id : null
}

/** P2P through someone named in text: said in the comment. */
export function viaComment(via: AccountRef | null, comment: string): string {
  return via && 'raw' in via ? [`P2P: ${via.raw}`, comment].filter(Boolean).join(' · ') : comment
}

export function common(form: TransactionFormState): TxCommonInput {
  return {
    timestamp: keepOriginalMoment(form.timestamp, form.originalTimestamp),
    comment: form.comment.trim(),
    categories: [...form.categories],
    flow_id: text(form.flowId),
    contact_id: text(form.contactId),
    // The backend turns `price` into USD at the rate of the operation's moment.
    price_usd: null,
    fx_override: null,
    price: text(form.price),
    price_currency: text(form.priceCurrency),
  }
}

export function details(form: TransactionFormState): KindDetails {
  return {
    cash_breakdown: null,
    unit_price: text(form.unitPrice),
    network: form.network || null,
    tx_hash: text(form.txHash),
    source_store_id: (form.categories.includes('cashback') && text(form.sourceStoreId)) || undefined,
  }
}
