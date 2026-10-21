import { computed, onScopeDispose, ref, watch } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { useLatestTx } from '@/widgets/transaction-details'
import { call } from '@/shared/api/ipc'
import { type EntityDialogKind, type EntityDialogRequest, entityDialog } from '@/shared/lib/entity-actions'
import { registerEntityOpener, useEntityNav } from '@/shared/lib/entity-link'
import { txDetailsTx } from '@/shared/lib/tx-details'
import { txId } from '@/shared/lib/transactions'

/**
 * The cards open at `?open=`: the top one is drawn, «Назад» or closing it
 * shows the one under it. An operation keeps its own sheet; every other kind
 * shares the generic card.
 */
export function useEntityHost() {
  const nav = useEntityNav()
  onScopeDispose(registerEntityOpener(nav.open))

  const top = nav.top
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
  /** The list's copy shows at once; the core's copy covers a link opened from an address. */
  const openedTx = computed<Transaction | null>(() => {
    if (!isTx.value) return null
    const cached = txDetailsTx.value
    if (cached && txId(cached) === top.value?.id) return cached
    return current.value?.transaction ?? null
  })
  const tx = useLatestTx(openedTx)

  const sheetOpen = (kind: 'tx' | 'other') =>
    computed({
      get: () => top.value !== null && (kind === 'tx') === isTx.value,
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
    card,
    current,
    tx,
    txOpen: sheetOpen('tx'),
    cardOpen: sheetOpen('other'),
    canGoBack: computed(() => nav.stack.value.length > 1),
    back: nav.back,
    request,
    goalOpen: dialogOpen('goal'),
    budgetOpen: dialogOpen('budget'),
    alertOpen: dialogOpen('alert'),
  }
}
