import { txKind, txMeta } from '@/shared/lib/transactions'

/**
 * The category used most often by operations of this kind (OpForm-1 · 1,
 * `Often: Groceries`); ties go to the one used last. `null` without history.
 */
export function frequentCategory(transactions: Transaction[], kind: TxKind): string | null {
  const counts = new Map<string, { count: number; order: number }>()
  transactions.forEach((tx, order) => {
    if (txKind(tx) !== kind) return
    for (const key of txMeta(tx).categories) {
      const entry = counts.get(key)
      counts.set(key, { count: (entry?.count ?? 0) + 1, order: entry?.order ?? order })
    }
  })
  let best: string | null = null
  let top = { count: 0, order: Number.POSITIVE_INFINITY }
  for (const [key, entry] of counts) {
    if (entry.count > top.count || (entry.count === top.count && entry.order < top.order)) {
      best = key
      top = entry
    }
  }
  return best
}
