import { dateBoundsError, keepOriginalMoment } from '@/shared/lib/datetime'
import { positiveAmount, required, type FieldError } from '@/shared/lib/form'

/** The kinds of the quick form: spending, income, a move between two wallets, an exchange of one currency for another. */
export type QuickKind = 'payment' | 'topup' | 'transfer' | 'convert'
export const QUICK_KINDS: readonly QuickKind[] = ['payment', 'topup', 'transfer', 'convert']

/** Kinds that name a second wallet instead of a category. */
export const isTwoWalletKind = (kind: QuickKind) => kind === 'transfer' || kind === 'convert'

export interface QuickFormState {
  kind: QuickKind
  storeId: string
  /** A transfer's or an exchange's destination wallet. */
  toStoreId: string
  /** Plain decimal, `12345.67`; for an exchange, what is given. */
  amount: string
  /** An exchange's received amount, in the destination wallet's currency. */
  toAmount: string
  /** The one category of an expense or income. */
  category: string
  /** Local `YYYY-MM-DDTHH:mm`; blank means now. */
  timestamp: string
  /** The counterparty's contact id; blank means nobody. */
  contactId: string
  /** A free note. */
  note: string
}

export type QuickField = 'amount' | 'storeId' | 'toStoreId' | 'toAmount' | 'category' | 'timestamp'

export function emptyQuickForm(kind: QuickKind = 'payment', storeId = ''): QuickFormState {
  return {
    kind,
    storeId,
    toStoreId: '',
    amount: '',
    toAmount: '',
    category: '',
    timestamp: '',
    contactId: '',
    note: '',
  }
}

/** What a field lacks, by the rules of the kind (rule 13: said under the field on save). */
export function quickErrors(form: QuickFormState): Record<QuickField, FieldError> {
  const twoWallets = isTwoWalletKind(form.kind)
  let toStoreId: FieldError = null
  if (twoWallets) {
    toStoreId = required(form.toStoreId, 'pickStore')
    if (!toStoreId && form.toStoreId === form.storeId) toStoreId = required(null, 'pickOtherWallet')
  }
  return {
    amount: positiveAmount(form.amount),
    storeId: required(form.storeId, 'pickStore'),
    toStoreId,
    toAmount: form.kind === 'convert' ? positiveAmount(form.toAmount) : null,
    category: twoWallets ? null : required(form.category, 'pick'),
    timestamp: dateBoundsError(form.timestamp),
  }
}

/** A series keeps the kind, the wallets, the category, the counterparty and the date; the amount starts blank. */
export function nextInSeries(form: QuickFormState): QuickFormState {
  return { ...form, amount: '', toAmount: '' }
}

/** The backend input of the form; `asset` is the wallet's own (see `use-quick-asset`), `toAsset` the destination wallet's (an exchange). */
export function quickFormToInput(
  form: QuickFormState,
  asset: AssetRef,
  toAsset: AssetRef | null = null,
): TransactionInput {
  const common: TxCommonInput = {
    timestamp: keepOriginalMoment(form.timestamp, null),
    comment: form.note.trim(),
    categories: isTwoWalletKind(form.kind) || !form.category ? [] : [form.category],
    flow_id: null,
    contact_id: form.contactId || null,
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
  if (form.kind === 'convert') {
    const received = toAsset ?? asset
    return {
      type: 'convert',
      common,
      from: { account: { store: form.storeId }, asset: ident, amount },
      to: {
        account: { store: form.toStoreId },
        asset: {
          symbol: received.symbol,
          name: received.name,
          coingecko_id: received.coingecko_id,
          chain: received.chain,
          contract: received.contract,
        },
        amount: form.toAmount.trim(),
      },
      fee: null,
      rate_override: null,
    }
  }
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
