/** Who and where: the two ends of a movement, the contacts it names, internal moves, unpriced legs. */

import { body, txKind, txMeta } from './access'
import { accountContactId, accountStoreId, txPrincipalLegs } from './legs'
import { txPair } from './pair'

/** The two ends of a movement, when the kind has them. */
export function txSides(tx: Transaction): { from: AccountRef | null; to: AccountRef | null } {
  switch (txKind(tx)) {
    case 'topup':
      return { from: body(tx, 'topup').from, to: null }
    case 'payment':
      return { from: null, to: body(tx, 'payment').to }
    case 'transfer': {
      const record = body(tx, 'transfer')
      return { from: record.from, to: record.to }
    }
    case 'convert': {
      const record = body(tx, 'convert')
      return { from: record.from.account, to: record.to.account }
    }
    default:
      return { from: null, to: null }
  }
}

/**
 * The two ends as a route reads them, «Тинькофф → Пятёрочка»: a payment leaves its own wallet and a
 * top-up lands in it, so that end is named too (`txSides` keeps only the far end, the one the form asks for).
 */
export function txRouteSides(tx: Transaction): { from: AccountRef | null; to: AccountRef | null } {
  const sides = txSides(tx)
  const kind = txKind(tx)
  if (kind === 'payment' && sides.to) return { from: { store: body(tx, 'payment').store_id }, to: sides.to }
  if (kind === 'topup' && sides.from) return { from: sides.from, to: { store: body(tx, 'topup').store_id } }
  return sides
}

/** Every contact the transaction names. */
export function txContactIds(tx: Transaction): string[] {
  const { from, to } = txSides(tx)
  const ids = [txMeta(tx).contact_id, accountContactId(from), accountContactId(to)]
  return [...new Set(ids.filter((id): id is string => Boolean(id)))]
}

/**
 * A transfer or conversion between two stores of one top-level wallet. Mirrors
 * `Transaction::is_internal`; `rootOf` maps a store id to its root's id.
 */
export function txIsInternal(tx: Transaction, rootOf: (id: string) => string): boolean {
  const kind = txKind(tx)
  if (kind !== 'transfer' && kind !== 'convert') return false
  const { from, to } = txSides(tx)
  const left = accountStoreId(from)
  const right = accountStoreId(to)
  return Boolean(left && right && rootOf(left) === rootOf(right))
}

/**
 * One asset moved between the user's own wallets: a transfer, or a «conversion» that gives and gets
 * the same asset (USD → USD). It reads as «Перевод», not as an exchange, and is neither in nor out.
 */
export function txIsMove(tx: Transaction): boolean {
  const kind = txKind(tx)
  if (kind !== 'transfer' && !(kind === 'convert' && !txPair(tx))) return false
  const { from, to } = txSides(tx)
  return Boolean(accountStoreId(from) && accountStoreId(to))
}

/** Whether no principal leg carries a price yet (the list marks it). */
export function txHasNoPrice(tx: Transaction): boolean {
  const legs = txPrincipalLegs(tx).filter((leg) => !leg.asset.contract?.startsWith('perp:'))
  return legs.length > 0 && legs.every((leg) => !leg.price_usd)
}
