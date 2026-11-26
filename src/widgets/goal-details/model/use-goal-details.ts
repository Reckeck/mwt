import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'

/**
 * The goal's progress, its wallet and the automation that fills it.
 */
export function useGoalDetails(
  goalId: MaybeRefOrGetter<string | null>,
  groups: MaybeRefOrGetter<RelationGroup[] | undefined>,
  open: MaybeRefOrGetter<boolean>,
) {
  const progress = useQuery({
    queryKey: ['goal-progress'],
    queryFn: () => call<GoalProgress[]>('get_goal_progress'),
    enabled: computed(() => toValue(open)),
  })
  const entry = computed(() => progress.data.value?.find((item) => item.goal.id === toValue(goalId)) ?? null)
  const store = computed(() => entry.value?.goal.scope.store_ids[0] ?? null)
  const tree = useStoreTree()
  /** The card's relation label when it opened from an address, else the wallet's name from the tree. */
  const storeName = computed(
    () =>
      toValue(groups)
        ?.find((group) => group.kind === 'stores')
        ?.items.find((item) => item.target.id === store.value)?.label ??
      flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === store.value)?.store.name ??
      '',
  )

  const filler = computed(() => {
    const kinds: RelationKind[] = ['automations', 'workflows']
    for (const kind of kinds) {
      const item = toValue(groups)
        ?.find((group) => group.kind === kind)
        ?.items.find((each) => !each.archived)
      if (item) return item
    }
    return null
  })

  return { progress, entry, store, storeName, filler }
}
