import { fiatRef } from '@/shared/lib/asset-ref'
import { positiveAmount, required } from '@/shared/lib/form'

/** The operation kinds an automation's template is edited as: an expense, an income, a move between wallets. */
export type TemplateKind = 'payment' | 'topup' | 'transfer'

export interface TemplateDraft {
  kind: TemplateKind
  amount: string
  category: string
  comment: string
  /** The receiving wallet of a transfer. */
  to: string
}

export type TemplateField = 'amount' | 'store' | 'to' | 'category'

export const TEMPLATE_KINDS: TemplateKind[] = ['payment', 'topup', 'transfer']

const sideStore = (side: AccountRef | null | undefined) => (side && 'store' in side ? side.store : '')

/** The kind of a stored template, or `null` for one this editor does not draw (an exchange, a trade). */
export function templateKindOf(transaction: Transaction | null | undefined): TemplateKind | null {
  if (!transaction) return null
  if ('payment' in transaction) return 'payment'
  if ('topup' in transaction) return 'topup'
  if ('transfer' in transaction) return 'transfer'
  return null
}

/** The start a wizard step or a preset gives (`WizardDraft`): a kind this editor lacks (an exchange) starts as an expense. */
export function templateStartOf(
  initial: { kind?: string; categories?: string[]; amount?: string; comment?: string; to?: string } | null | undefined,
): Partial<TemplateDraft> | null {
  if (!initial) return null
  const kind = TEMPLATE_KINDS.find((candidate) => candidate === initial.kind) ?? 'payment'
  return {
    kind,
    category: initial.categories?.[0] ?? '',
    amount: initial.amount ?? '',
    comment: initial.comment ?? '',
    to: initial.to ?? '',
  }
}

/** What the form starts as: a stored template, else the given start, else a blank expense. */
export function templateDraftOf(
  transaction: Transaction | null | undefined,
  initial?: Partial<TemplateDraft> | null,
): TemplateDraft {
  const blank: TemplateDraft = { kind: 'payment', amount: '', category: '', comment: '', to: '', ...initial }
  const kind = templateKindOf(transaction)
  if (!transaction || !kind) return blank
  const body =
    'payment' in transaction
      ? transaction.payment
      : 'topup' in transaction
        ? transaction.topup
        : 'transfer' in transaction
          ? transaction.transfer
          : null
  if (!body) return blank
  return {
    kind,
    amount: body.amount,
    category: body.meta.categories[0] ?? '',
    comment: body.meta.comment,
    to: 'transfer' in transaction ? sideStore(transaction.transfer.to) : '',
  }
}

/** The messages by field (a transfer to its own wallet is the one rule this adds); empty when the draft can be saved. */
export function templateErrors(draft: TemplateDraft, storeId: string): Partial<Record<TemplateField, string>> {
  const found: Record<TemplateField, string | null> = {
    amount: positiveAmount(draft.amount),
    store: required(storeId),
    to: draft.kind === 'transfer' ? required(draft.to) : null,
    category: draft.kind === 'transfer' ? null : required(draft.category),
  }
  return Object.fromEntries(Object.entries(found).filter(([, message]) => message)) as Partial<
    Record<TemplateField, string>
  >
}

/** The backend input of the template; `asset` is the wallet's own asset (the base currency when the wallet holds none). */
export function templateToInput(
  draft: TemplateDraft,
  storeId: string,
  asset: AssetRef | null,
  baseCurrency: string,
  propertyId = '',
): TransactionInput {
  const ref = asset ?? fiatRef(baseCurrency)
  const ident: AssetIdent = {
    symbol: ref.symbol,
    name: ref.name,
    coingecko_id: ref.coingecko_id,
    chain: ref.chain,
    contract: ref.contract,
  }
  const common: TxCommonInput = {
    timestamp: null,
    comment: draft.comment.trim(),
    categories: draft.kind === 'transfer' || !draft.category ? [] : [draft.category],
    flow_id: null,
    contact_id: null,
    price_usd: null,
    fx_override: null,
    price: null,
    price_currency: null,
    ...(propertyId ? { property_id: propertyId } : {}),
  }
  const amount = draft.amount.trim().replace(',', '.')
  const details: KindDetails = { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }
  if (draft.kind === 'transfer') {
    return {
      type: 'transfer',
      common,
      from: { store: storeId },
      to: { store: draft.to },
      asset: ident,
      amount,
      fee: null,
      network: null,
      tx_hash: null,
      debt: null,
    }
  }
  if (draft.kind === 'topup')
    return { type: 'topup', common, store_id: storeId, asset: ident, amount, fee: null, from: null, details }
  return { type: 'payment', common, store_id: storeId, asset: ident, amount, fee: null, to: null, details }
}

/** The asset a stored template is written in, so a refresh compares like with like. */
export function templateAssetOf(transaction: Transaction | null | undefined): AssetRef | null {
  if (!transaction) return null
  if ('payment' in transaction) return transaction.payment.asset
  if ('topup' in transaction) return transaction.topup.asset
  if ('transfer' in transaction) return transaction.transfer.asset
  return null
}
