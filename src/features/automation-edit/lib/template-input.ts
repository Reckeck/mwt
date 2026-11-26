import { i18n } from '@/shared/config/i18n'
import { fiatRef } from '@/shared/lib/asset-ref'
import { firstError, positiveAmount, required } from '@/shared/lib/form'

/** The operation kinds an automation's template is edited as: an expense, an income, a move between wallets. */
export type TemplateKind = 'payment' | 'topup' | 'transfer'

/** What a stored template holds that the form has no field for; a save writes it back as it was. */
export interface TemplateKept {
  /** The asset the template is written in. */
  asset: AssetRef
  categories: string[]
  contactId: string | null
  flowId: string | null
  price: PriceStamp
  fee: Fee | null
  /** The other side of a payment (`to`) or of a top-up (`from`). */
  counter: AccountRef | null
  details: KindDetails | null
  period?: OpPeriod
  spread?: Spread
  network: Chain | null
  txHash: string | null
  debt: DebtEffect | null
}

export interface TemplateDraft {
  kind: TemplateKind
  amount: string
  category: string
  comment: string
  /** The receiving wallet of a transfer. */
  to: string
  /** Set when the draft starts from a stored template; a new template has none. */
  kept?: TemplateKept
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
    kept: keptOf(transaction),
  }
}

function keptOf(transaction: Transaction): TemplateKept | undefined {
  if ('payment' in transaction)
    return keptFrom(transaction.payment, transaction.payment.to, transaction.payment.details)
  if ('topup' in transaction) return keptFrom(transaction.topup, transaction.topup.from, transaction.topup.details)
  if ('transfer' in transaction) {
    const { network, tx_hash: txHash, debt } = transaction.transfer
    return { ...keptFrom(transaction.transfer, null, null), network, txHash, debt }
  }
  return undefined
}

function keptFrom(
  body: { meta: TxMeta; asset: AssetRef; fee: Fee | null; price: PriceStamp },
  counter: AccountRef | null,
  details: KindDetails | null,
): TemplateKept {
  const { meta } = body
  return {
    asset: body.asset,
    categories: meta.categories,
    contactId: meta.contact_id,
    flowId: meta.flow_id,
    price: body.price,
    fee: body.fee,
    counter,
    details,
    period: meta.period,
    spread: meta.spread,
    network: null,
    txHash: null,
    debt: null,
  }
}

/** The draft of another kind: what the old kind kept (the other side, the fee, the details) no longer fits, the asset does. */
export function templateRetyped(draft: TemplateDraft, kind: TemplateKind): void {
  draft.kind = kind
  draft.category = ''
  if (draft.kept)
    draft.kept = {
      ...draft.kept,
      categories: [],
      counter: null,
      details: null,
      network: null,
      txHash: null,
      debt: null,
    }
}

/** The messages by field (a transfer to its own wallet is the one rule this adds); empty when the draft can be saved. */
export function templateErrors(draft: TemplateDraft, storeId: string): Partial<Record<TemplateField, string>> {
  const found: Record<TemplateField, string | null> = {
    amount: positiveAmount(draft.amount),
    store: required(storeId),
    to:
      draft.kind === 'transfer'
        ? firstError(required(draft.to), draft.to === storeId ? i18n.global.t('automation.template.toSelf') : null)
        : null,
    category: draft.kind === 'transfer' ? null : required(draft.category),
  }
  return Object.fromEntries(Object.entries(found).filter(([, message]) => message)) as Partial<
    Record<TemplateField, string>
  >
}

const identOf = (ref: AssetRef): AssetIdent => ({
  symbol: ref.symbol,
  name: ref.name,
  coingecko_id: ref.coingecko_id,
  chain: ref.chain,
  contract: ref.contract,
})

/** The common part of the input: what the form edits, then what the stored template kept. */
function commonOf(draft: TemplateDraft, propertyId: string): TxCommonInput {
  const kept = draft.kept
  // The category the user left alone keeps its whole list; a picked one replaces it.
  const categories =
    draft.kind === 'transfer' || !draft.category
      ? []
      : kept?.categories[0] === draft.category
        ? kept.categories
        : [draft.category]
  return {
    timestamp: null,
    comment: draft.comment.trim(),
    categories,
    flow_id: kept?.flowId ?? null,
    contact_id: kept?.contactId ?? null,
    price_usd: kept?.price.price_usd ?? null,
    fx_override: kept?.price.fx_override ?? null,
    price: kept?.price.price_base ?? null,
    price_currency: kept?.price.price_base_currency ?? null,
    ...(propertyId ? { property_id: propertyId } : {}),
    ...(kept?.period ? { period: kept.period } : {}),
    ...(kept?.spread ? { spread: kept.spread } : {}),
  }
}

const feeOf = (kept: TemplateKept | undefined): FeeInput | null =>
  kept?.fee ? { asset: identOf(kept.fee.asset), amount: kept.fee.amount } : null

/**
 * The backend input of the template. The asset is the stored template's own (`draft.kept`); a new
 * template takes `asset`, the wallet's own (the base currency when the wallet holds none).
 */
export function templateToInput(
  draft: TemplateDraft,
  storeId: string,
  asset: AssetRef | null,
  baseCurrency: string,
  propertyId = '',
): TransactionInput {
  const ident = identOf(draft.kept?.asset ?? asset ?? fiatRef(baseCurrency))
  const common = commonOf(draft, propertyId)
  const kept = draft.kept
  const amount = draft.amount.trim().replace(',', '.')
  const fee = feeOf(kept)
  const details: KindDetails = kept?.details ?? { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }
  if (draft.kind === 'transfer') {
    return {
      type: 'transfer',
      common,
      from: { store: storeId },
      to: { store: draft.to },
      asset: ident,
      amount,
      fee,
      network: kept?.network ?? null,
      tx_hash: kept?.txHash ?? null,
      debt: kept?.debt ?? null,
    }
  }
  if (draft.kind === 'topup')
    return { type: 'topup', common, store_id: storeId, asset: ident, amount, fee, from: kept?.counter ?? null, details }
  return { type: 'payment', common, store_id: storeId, asset: ident, amount, fee, to: kept?.counter ?? null, details }
}

/** The asset a stored template is written in, so a refresh compares like with like. */
export function templateAssetOf(transaction: Transaction | null | undefined): AssetRef | null {
  if (!transaction) return null
  if ('payment' in transaction) return transaction.payment.asset
  if ('topup' in transaction) return transaction.topup.asset
  if ('transfer' in transaction) return transaction.transfer.asset
  return null
}
