import { computed, onScopeDispose, ref, watch } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { useLatestTx } from '@/widgets/transaction-details'
import { call } from '@/shared/api/ipc'
import { type EntityDialogKind, type EntityDialogRequest, entityDialog } from '@/shared/lib/entity-actions'
import { registerEntityOpener, useEntityNav } from '@/shared/lib/entity-link'
import { rememberEntity } from '@/shared/lib/recent-entities'
import { txDetailsTx } from '@/shared/lib/tx-details'
import { formatDayMonthShort } from '@/shared/lib/format'
import { txId, txMeta } from '@/shared/lib/transactions'

/**
 * The cards open at `?open=`: the top one is drawn, «Назад» or closing it
 * shows the one under it. An operation, a goal and an automation keep their own sheets;
 * every other kind shares the generic card.
 */
export function useEntityHost() {
  const nav = useEntityNav()
  onScopeDispose(registerEntityOpener(nav.open))

  const top = nav.top
  watch(top, (target) => {
    if (target) rememberEntity(target)
  })
  const card = useQuery({
    queryKey: computed(() => ['entity-card', top.value?.kind, top.value?.id]),
    queryFn: () => call<EntityCard>('get_entity_card', { target: top.value }),
    enabled: computed(() => top.value !== null),
    placeholderData: keepPreviousData,
  })
  /** A placeholder from the previous card is not this card. */
  const current = computed(() => {
    const data = card.data.value
    const target = top.value
    return data && target && data.target.kind === target.kind && data.target.id === target.id ? data : null
  })

  const isTx = computed(() => top.value?.kind === 'tx')
  /**
   * The list's copy shows at once; the core's copy replaces it as soon as it is read, so a
   * saved edit shows its new values, and covers a link opened from an address.
   */
  const openedTx = computed<Transaction | null>(() => {
    if (!isTx.value) return null
    const cached = txDetailsTx.value
    const fresh = current.value?.transaction
    if (fresh) return fresh
    if (cached && txId(cached) === top.value?.id && !current.value?.missing) return cached
    return null
  })
  /** A deleted operation has no card to show: the one under it (or the page) comes back. */
  watch(
    () => isTx.value && current.value?.missing === true,
    (gone) => {
      if (gone) nav.back()
    },
  )
  const tx = useLatestTx(openedTx)
  /** Ctrl K lists an operation by its comment and day («Маме до зарплаты · 20 окт»). */
  watch(tx, (shown) => {
    const meta = shown ? txMeta(shown) : null
    if (meta?.comment && top.value?.kind === 'tx')
      rememberEntity(top.value, `${meta.comment} · ${formatDayMonthShort(meta.timestamp)}`)
  })

  /** An operation, a goal and an automation have their own sheets; the rest share the generic card. */
  const shownSheet = computed(() => {
    const kind = top.value?.kind
    if (!kind) return null
    return kind === 'tx' || kind === 'goal' || kind === 'automation' ? kind : 'other'
  })
  const sheetOpen = (kind: 'tx' | 'goal' | 'automation' | 'other') =>
    computed({
      get: () => shownSheet.value === kind,
      set: (value: boolean) => {
        if (!value) nav.back()
      },
    })

  /**
   * The dialog asked for by a card action. The dialogs stay mounted (they
   * fill their fields when `open` turns on); the request stays after closing
   * so the closing dialog keeps its content.
   */
  const request = ref<EntityDialogRequest | null>(null)
  const closed = ref(false)
  watch(entityDialog, (next) => {
    if (!next) return
    request.value = next
    closed.value = false
    entityDialog.value = null
  })
  const dialogOpen = (kind: EntityDialogKind) =>
    computed({
      get: () => request.value?.dialog === kind && !closed.value,
      set: (value: boolean) => {
        closed.value = !value
      },
    })

  return {
    top,
    card,
    current,
    tx,
    txOpen: sheetOpen('tx'),
    goalCardOpen: sheetOpen('goal'),
    automationOpen: sheetOpen('automation'),
    cardOpen: sheetOpen('other'),
    request,
    goalOpen: dialogOpen('goal'),
    budgetOpen: dialogOpen('budget'),
    alertOpen: dialogOpen('alert'),
  }
}
