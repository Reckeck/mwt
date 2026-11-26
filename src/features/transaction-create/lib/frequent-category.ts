import { txKind, txMeta } from '@/shared/lib/transactions'

/** How often a category was used by operations of one kind, and where it first appears (0 = the newest). */
export interface CategoryUse {
  count: number
  order: number
}

/** The use of every category in `transactions` (newest first) by operations of this kind. */
export function categoryUsage(transactions: Transaction[], kind: TxKind): Map<string, CategoryUse> {
  const counts = new Map<string, CategoryUse>()
  transactions.forEach((tx, order) => {
    if (txKind(tx) !== kind) return
    for (const key of txMeta(tx).categories) {
      const entry = counts.get(key)
      counts.set(key, { count: (entry?.count ?? 0) + 1, order: entry?.order ?? order })
    }
  })
  return counts
}

/** The more used first; ties go to the one used last; unused ones keep their order. */
export function byUsage<Option extends { value: string }>(
  options: Option[],
  usage: Map<string, CategoryUse>,
): Option[] {
  const rank = (option: Option) => usage.get(option.value)
  return options
    .map((option, index) => ({ option, index }))
    .sort((left, right) => {
      const leftUse = rank(left.option)
      const rightUse = rank(right.option)
      if (leftUse && rightUse)
        return rightUse.count - leftUse.count || leftUse.order - rightUse.order || left.index - right.index
      if (leftUse || rightUse) return leftUse ? -1 : 1
      return left.index - right.index
    })
    .map(({ option }) => option)
}

/**
 * The category used most often by operations of this kind (OpForm-1 · 1,
 * `Often: Groceries`); ties go to the one used last. `null` without history.
 */
export function frequentCategory(transactions: Transaction[], kind: TxKind): string | null {
  let best: string | null = null
  let top = { count: 0, order: Number.POSITIVE_INFINITY }
  for (const [key, entry] of categoryUsage(transactions, kind)) {
    if (entry.count > top.count || (entry.count === top.count && entry.order < top.order)) {
      best = key
      top = entry
    }
  }
  return best
}
