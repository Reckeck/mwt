import { type MaybeRefOrGetter, computed, onScopeDispose, toValue } from 'vue'

import { openTxDetails, txDetailsList } from '@/shared/lib/tx-details'
import { txId } from '@/shared/lib/transactions'

const typing = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))

/**
 * ‹ › in the card's subtitle (Paths 1 · 6): the rows of the list it was opened
 * from, without closing it. ← and → do the same while the card is open.
 */
export function useTxNeighbors(tx: MaybeRefOrGetter<Transaction | null>, open: MaybeRefOrGetter<boolean>) {
  const index = computed(() => {
    const current = toValue(tx)
    return current ? txDetailsList.value.findIndex((row) => txId(row) === txId(current)) : -1
  })
  const prev = computed(() => (index.value > 0 ? txDetailsList.value[index.value - 1] : null))
  const next = computed(() => (index.value >= 0 ? (txDetailsList.value[index.value + 1] ?? null) : null))

  function go(step: -1 | 1) {
    const target = step < 0 ? prev.value : next.value
    if (target) openTxDetails(target, txDetailsList.value)
  }

  const listener = (event: KeyboardEvent) => {
    if (!toValue(open) || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return
    if (typing(event.target) || document.querySelector('[role="menu"], [role="alertdialog"]')) return
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    event.preventDefault()
    go(event.key === 'ArrowLeft' ? -1 : 1)
  }
  window.addEventListener('keydown', listener)
  onScopeDispose(() => window.removeEventListener('keydown', listener))

  return { prev, next, go, shown: computed(() => index.value >= 0 && txDetailsList.value.length > 1) }
}
