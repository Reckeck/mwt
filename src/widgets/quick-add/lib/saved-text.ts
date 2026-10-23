import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { txKind, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'

type Translate = (key: string, named?: Record<string, unknown>) => string

const SAID_KINDS = new Set(['payment', 'convert', 'transfer', 'topup'])

/** «Оплата 340 ₽ · Продукты сохранена»: what was just written, in the toast after a save. */
export function savedText(tx: Transaction, categories: Category[] | undefined, t: Translate): string {
  const kind = txKind(tx)
  const said = SAID_KINDS.has(kind) ? kind : 'other'
  const leg = txPrincipalLegs(tx)[0]
  const amount = leg ? formatMoney(Math.abs(Number(leg.amount)), { currency: leg.asset.symbol }) : ''
  const category = txMeta(tx).categories[0]
  const what = [
    [said === 'other' ? '' : t(`tx.kindTab.${kind}`), amount].filter(Boolean).join(' '),
    category ? categoryKeyLabel(category, categories, t) : '',
  ]
    .filter(Boolean)
    .join(' · ')
  return t(`quickAdd.saved.${said}`, { what })
}
