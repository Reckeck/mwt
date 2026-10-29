import { useI18n } from 'vue-i18n'

import { accountContactId, txIsMove, txKind, txMeta, txPair, txPrincipalLegs, txSides } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

/** What a row and its card are called, and the category the title already names (its badge is not repeated). */
export interface TxTitle {
  text: string
  category: string | null
}

export interface TxTitleNames {
  categoryName: (key: string) => string
  contactName: (id: string) => string
  sideLabel: (side: AccountRef | null) => string
  /** `Transfer`, `Correction`: the kind's noun. */
  kindNoun: (kind: TxKind) => string
}

/** Kinds whose instrument says more than the kind's name («BTCUSDT» of a futures deal). */
const BY_ASSET: TxKind[] = ['trade_spot', 'trade_futures', 'defi']

/**
 * By meaning, not by currency: «RUB → USDT» for an exchange, `Transfer` for one asset moved,
 * else who (`Mom`) or what for (`Salary`), else the far side (`Pyaterochka`). The currency
 * stays with the sum.
 */
export function txTitle(tx: Transaction, names: TxTitleNames): TxTitle {
  const kind = txKind(tx)
  const pair = txPair(tx)
  if (pair) return { text: `${pair.out.symbol} → ${pair.into.symbol}`, category: null }
  if (txIsMove(tx)) return { text: names.kindNoun('transfer'), category: null }
  const meta = txMeta(tx)
  const { from, to } = txSides(tx)
  const contact = meta.contact_id || accountContactId(from) || accountContactId(to)
  if (contact) return { text: names.contactName(contact), category: null }
  const category = meta.categories[0]
  if (category) return { text: names.categoryName(category), category }
  const far = [to, from].find((side) => side && !('store' in side))
  if (far) return { text: names.sideLabel(far), category: null }
  if (BY_ASSET.includes(kind))
    return {
      text: txPrincipalLegs(tx)
        .map((leg) => leg.asset.symbol)
        .join(' → '),
      category: null,
    }
  return { text: names.kindNoun(kind), category: null }
}

/** `txTitle` with this vault's names. */
export function useTxTitle(): (tx: Transaction) => TxTitle {
  const { t } = useI18n()
  const context = useTxContext()
  const names: TxTitleNames = { ...context, kindNoun: (kind) => t(`tx.kindNoun.${kind}`) }
  return (tx) => txTitle(tx, names)
}
