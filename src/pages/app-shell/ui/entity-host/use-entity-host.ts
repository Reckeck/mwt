import { computed, onScopeDispose, ref, watch } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { type EntityDialogKind, type EntityDialogRequest, entityDialog } from '@/shared/lib/entity-actions'
import { registerEntityOpener, useEntityNav } from '@/shared/lib/entity-link'
import { rememberEntity } from '@/shared/lib/recent-entities'

/**
 * The cards open at `?open=`: the top one is drawn, `Back` or closing it
 * shows the one under it.
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

  /** A deleted operation has no card to show: the one under it (or the page) comes back. */
  watch(
    () => top.value?.kind === 'tx' && current.value?.missing === true,
    (gone) => {
      if (gone) nav.back()
    },
  )
  /** Until the screens return in W4 every kind shares the generic card. */
  const cardOpen = computed({
    get: () => top.value !== null,
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
    cardOpen,
    request,
    goalOpen: dialogOpen('goal'),
    budgetOpen: dialogOpen('budget'),
    alertOpen: dialogOpen('alert'),
  }
}
