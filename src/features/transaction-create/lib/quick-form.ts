import { dateBoundsError, keepOriginalMoment } from '@/shared/lib/datetime'
import { positiveAmount, required, type FieldError } from '@/shared/lib/form'

/** The three kinds of the quick form: spending, income, a move between two wallets. */
export type QuickKind = 'payment' | 'topup' | 'transfer'
export const QUICK_KINDS: readonly QuickKind[] = ['payment', 'topup', 'transfer']

export interface QuickFormState {
  kind: QuickKind
  storeId: string
  /** A transfer's destination wallet. */
  toStoreId: string
  /** Plain decimal, `12345.67`. */
  amount: string
  /** The one category of an expense or income. */
  category: string
  /** Local `YYYY-MM-DDTHH:mm`; blank means now. */
  timestamp: string
}

export type QuickField = 'amount' | 'storeId' | 'toStoreId' | 'category' | 'timestamp'

export function emptyQuickForm(kind: QuickKind = 'payment', storeId = ''): QuickFormState {
  return { kind, storeId, toStoreId: '', amount: '', category: '', timestamp: '' }
}

/** What a field lacks, by the rules of the kind (rule 13: said under the field on save). */
export function quickErrors(form: QuickFormState): Record<QuickField, FieldError> {
  const transfer = form.kind === 'transfer'
  let toStoreId: FieldError = null
  if (transfer) {
    toStoreId = required(form.toStoreId, 'pickStore')
    if (!toStoreId && form.toStoreId === form.storeId) toStoreId = required(null, 'pickOtherWallet')
  }
  return {
    amount: positiveAmount(form.amount),
    storeId: required(form.storeId, 'pickStore'),
    toStoreId,
    category: transfer ? null : required(form.category, 'pick'),
    timestamp: dateBoundsError(form.timestamp),
  }
}

/** A series keeps the kind, the wallets, the category and the date; the amount starts blank. */
export function nextInSeries(form: QuickFormState): QuickFormState {
  return { ...form, amount: '' }
}

/** The backend input of the form; `asset` is the wallet's own (see `use-quick-asset`). */
export function quickFormToInput(form: QuickFormState, asset: AssetRef): TransactionInput {
  const common: TxCommonInput = {
    timestamp: keepOriginalMoment(form.timestamp, null),
    comment: '',
    categories: form.kind === 'transfer' || !form.category ? [] : [form.category],
    flow_id: null,
    contact_id: null,
    price_usd: null,
    fx_override: null,
    price: null,
    price_currency: null,
  }
  const ident: AssetIdent = {
    symbol: asset.symbol,
    name: asset.name,
    coingecko_id: asset.coingecko_id,
    chain: asset.chain,
    contract: asset.contract,
  }
  const amount = form.amount.trim()
  const details: KindDetails = { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }
  if (form.kind === 'transfer') {
    return {
      type: 'transfer',
      common,
      fee: null,
      from: { store: form.storeId },
      to: { store: form.toStoreId },
      asset: ident,
      amount,
      network: null,
      tx_hash: null,
      debt: null,
    }
  }
  if (form.kind === 'topup') {
    return { type: 'topup', common, fee: null, store_id: form.storeId, asset: ident, amount, from: null, details }
  }
  return { type: 'payment', common, fee: null, store_id: form.storeId, asset: ident, amount, to: null, details }
}
