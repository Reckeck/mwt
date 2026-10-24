import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { useWorkflowActions } from '@/features/workflow-manage'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Under `workflows`: every workflow write — a run, a switch, a save — refreshes the card. */
export const automationCardKey = (id: string | null) => [...WORKFLOW_KEYS.list, 'card', id]

/**
 * The card of a scheme or a repeat (Auto-1 · 3) and what it does: switch it,
 * run it now, delete it. Both are workflows, so the workflow actions serve.
 */
export function useAutomationDetails(id: MaybeRefOrGetter<string | null>, open: MaybeRefOrGetter<boolean>) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const card = useQuery({
    queryKey: computed(() => automationCardKey(toValue(id))),
    queryFn: () => call<AutomationCard>('get_automation_card', { id: toValue(id) }),
    enabled: computed(() => toValue(open) && toValue(id) !== null),
    // no placeholder: another automation's card must not show under this one's name
  })
  const current = computed(() => (card.data.value?.id === toValue(id) ? card.data.value : null))

  const tree = useStoreTree()
  const stores = computed(
    () => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name])),
  )
  const categories = useCategories()
  const names = {
    store: (storeId: string) => stores.value.get(storeId) ?? '',
    category: (key: string) => categoryKeyLabel(key, categories.data.value, t),
  }

  const actions = useWorkflowActions()
  function setActive(active: boolean) {
    const shown = current.value
    if (!shown) return
    queryClient.setQueryData<AutomationCard>(automationCardKey(shown.id), { ...shown, active })
    actions.setActive.mutate(
      { id: shown.id, active },
      { onError: () => void queryClient.invalidateQueries({ queryKey: automationCardKey(shown.id) }) },
    )
  }
  const runNow = () => current.value && actions.runNow.mutate({ id: current.value.id })
  const remove = (onDone: () => void) => current.value && actions.remove.mutate(current.value.id, { onSuccess: onDone })

  return {
    card,
    current,
    names,
    setActive,
    runNow,
    running: actions.runNow.isPending,
    remove,
    removing: actions.remove.isPending,
  }
}
