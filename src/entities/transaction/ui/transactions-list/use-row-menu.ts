import { type Ref, computed, ref } from 'vue'
import { onLongPress, useEventListener } from '@vueuse/core'

import type { ActionSheetProps } from '@/shared/ui/action-menu'
import type { UiAction } from '@/shared/ui/context-menu'
import { formatDayMonth, formatMoney } from '@/shared/lib/format'
import { useCoarsePointer } from '@/shared/lib/phone'
import { txId, txMeta, txPrincipalLegs, txSides } from '@/shared/lib/transactions'
import type { TxContext } from '@/shared/lib/tx-context'

/** The keys a focused row answers to, by action id; the menu shows them next to its items. */
const ROW_KEYS: { key: string; ctrl?: boolean; action: string }[] = [
  { key: ' ', action: 'glance' },
  { key: 'Enter', action: 'edit' },
  { key: 'd', ctrl: true, action: 'duplicate' },
  { key: 'Delete', action: 'delete' },
]

/** How long a finger holds a row before its actions come up. */
const HOLD_MS = 500

/**
 * The row menu beyond the right click (Paths 2 · 11): a long press on a phone
 * brings the same actions up as a sheet, and a focused row answers to the keys
 * the menu shows (Space, Enter, Ctrl D, Del).
 */
export function useRowMenu(options: {
  root: Ref<HTMLElement | null>
  rows: () => Transaction[]
  actions: (tx: Transaction) => UiAction[]
  enabled: () => boolean
  context: TxContext
}) {
  const coarse = useCoarsePointer()
  const sheetTx = ref<Transaction | null>(null)
  let heldAt = 0

  const rowOf = (target: EventTarget | null) => {
    const id = (target as Element | null)?.closest<HTMLElement>('[data-tx-row]')?.dataset.txRow
    return id ? options.rows().find((entry) => txId(entry) === id) : undefined
  }

  onLongPress(
    options.root,
    (event) => {
      if (!coarse.value || !options.enabled()) return
      const tx = rowOf(event.target)
      if (!tx) return
      heldAt = Date.now()
      sheetTx.value = tx
    },
    { delay: HOLD_MS, modifiers: { stop: false } },
  )
  // The finger lifting after a hold is not a tap: the row must not open too.
  useEventListener(
    options.root,
    'click',
    (event: MouseEvent) => {
      if (Date.now() - heldAt > 800) return
      event.preventDefault()
      event.stopPropagation()
    },
    { capture: true },
  )

  useEventListener(options.root, 'keydown', (event: KeyboardEvent) => {
    if (!options.enabled() || !(event.target as Element | null)?.matches('[data-tx-open]')) return
    const tx = rowOf(event.target)
    const match = ROW_KEYS.find(
      (entry) => entry.key === event.key && Boolean(entry.ctrl) === (event.ctrlKey || event.metaKey),
    )
    const action =
      tx && match ? options.actions(tx).find((entry) => entry.id === match.action && !entry.hidden) : undefined
    if (!action?.run) return
    event.preventDefault()
    action.run()
  })

  const sheetOpen = computed({
    get: () => sheetTx.value !== null,
    set: (value: boolean) => {
      if (!value) sheetTx.value = null
    },
  })
  const sheetActions = computed(() => (sheetTx.value ? options.actions(sheetTx.value) : []))

  /** «Маме до зарплаты · 20 окт · Тинькофф → Мама · −5 000 ₽»: what the sheet acts on. */
  function sheetHead(tx: Transaction): Omit<ActionSheetProps, 'actions'> {
    const meta = txMeta(tx)
    const sides = txSides(tx)
    const route = [sides.from, sides.to].filter(Boolean).map((side) => options.context.sideLabel(side))
    const leg = txPrincipalLegs(tx)[0]
    return {
      title: meta.comment || leg?.asset.symbol || '',
      description: [formatDayMonth(meta.timestamp), route.join(' → ')].filter(Boolean).join(' · '),
      // money: plain — the sheet's value is a string prop of ActionSheet
      value: leg ? formatMoney(leg.amount, { currency: leg.asset.symbol, signed: true, decimals: 0 }) : '',
    }
  }

  return { coarse, sheetTx, sheetOpen, sheetActions, sheetHead }
}
