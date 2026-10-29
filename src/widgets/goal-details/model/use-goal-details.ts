import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'

/**
 * The goal's progress, the automation that fills it and the wallet `Top up`
 * sends from: the first everyday wallet outside the goal.
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
  const storeName = computed(
    () =>
      toValue(groups)
        ?.find((group) => group.kind === 'stores')
        ?.items.find((item) => item.target.id === store.value)?.label ?? '',
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

  const tree = useStoreTree()
  const source = computed(() => {
    const own = new Set(entry.value?.goal.scope.store_ids ?? [])
    const nodes = flattenStoreTree(tree.data.value ?? []).filter(
      (node) => !node.store.archived && !own.has(node.store.id),
    )
    return (nodes.find((node) => node.store.purpose === 'everyday') ?? nodes[0])?.store.id ?? ''
  })

  return { progress, entry, store, storeName, filler, source }
}
