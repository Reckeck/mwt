import type { PendingAttachment } from '@/features/attachment-edit'
import { quickDebtEffect } from '@/features/debt-manage'
import type { QuickKind } from '@/features/transaction-create'
import { positiveAmount } from '@/shared/lib/form'

/** The rare fields of the quick form, behind «More». All of them are optional. */
export interface QuickMore {
  /** The fee, in the wallet's own asset. */
  fee: string
  /** Who was paid (an expense) or who paid (an income), as free text. */
  payee: string
  /** A property wallet the operation belongs to. */
  propertyId: string
  /** A `Chain` name, or blank for none. */
  network: string
  txHash: string
  /** A transfer that comes back later. */
  asDebt: boolean
  files: PendingAttachment[]
}

export type QuickMoreField = 'fee' | 'payee' | 'property' | 'network' | 'debt' | 'files'

export function emptyQuickMore(): QuickMore {
  return { fee: '', payee: '', propertyId: '', network: '', txHash: '', asDebt: false, files: [] }
}

/** Which rare fields a kind has: a payee only where money meets someone, a loan only on a transfer. */
export function moreFieldsOf(kind: QuickKind): QuickMoreField[] {
  if (kind === 'convert') return ['fee', 'property', 'files']
  if (kind === 'transfer') return ['fee', 'network', 'debt', 'property', 'files']
  return ['fee', 'payee', 'network', 'property', 'files']
}

/** How many rare fields hold a value: the badge of the «More» button. */
export function filledMoreCount(more: QuickMore, kind: QuickKind): number {
  const filled: Record<QuickMoreField, boolean> = {
    fee: more.fee.trim() !== '',
    payee: more.payee.trim() !== '',
    property: more.propertyId !== '',
    network: more.network !== '' || more.txHash.trim() !== '',
    debt: more.asDebt,
    files: more.files.length > 0,
  }
  return moreFieldsOf(kind).filter((field) => filled[field]).length
}

/** What is wrong in the rare fields (said on save, rule 13); `null` when all is well. */
export function moreError(more: QuickMore): string | null {
  return more.fee.trim() === '' ? null : positiveAmount(more.fee)
}

/** The backend input with the rare fields put in. A field the kind does not have is left out. */
export function applyMore(
  input: TransactionInput,
  more: QuickMore,
  asset: AssetRef,
  kind: QuickKind,
): TransactionInput {
  const fields = moreFieldsOf(kind)
  const has = (field: QuickMoreField) => fields.includes(field)
  const ident: AssetIdent = {
    symbol: asset.symbol,
    name: asset.name,
    coingecko_id: asset.coingecko_id,
    chain: asset.chain,
    contract: asset.contract,
  }
  const fee: FeeInput | null = has('fee') && more.fee.trim() ? { asset: ident, amount: more.fee.trim() } : null
  const network = has('network') && more.network ? (more.network as Chain) : null
  const txHash = has('network') && more.txHash.trim() ? more.txHash.trim() : null
  const payee: AccountRef | null = has('payee') && more.payee.trim() ? { raw: more.payee.trim() } : null
  const common: TxCommonInput = { ...input.common }
  if (has('property') && more.propertyId) common.property_id = more.propertyId

  switch (input.type) {
    case 'payment':
      return { ...input, common, fee, to: payee, details: { ...input.details, network, tx_hash: txHash } }
    case 'topup':
      return { ...input, common, fee, from: payee, details: { ...input.details, network, tx_hash: txHash } }
    case 'transfer':
      return {
        ...input,
        common,
        fee,
        network,
        tx_hash: txHash,
        debt: quickDebtEffect(kind, has('debt') && more.asDebt),
      }
    case 'convert':
      return { ...input, common, fee }
    default:
      return { ...input, common }
  }
}
