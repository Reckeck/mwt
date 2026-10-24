import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

import { type MineFilter, isMineFilter } from '../lib/mine-view'

/** Query key of `list_my_automations`: under `workflows`, and in PLANNING_KEYS, so every write there refreshes it. */
export const MINE_KEY = [...WORKFLOW_KEYS.list, 'mine']

/** «Мои» (Auto-1 · 1): the list, the chip that narrows it (`?type=`), the switch of a row. */
export function useMine() {
  const route = useRoute()
  const router = useRouter()
  const queryClient = useQueryClient()

  const list = useQuery({ queryKey: MINE_KEY, queryFn: () => call<MineItem[]>('list_my_automations') })
  const tree = useStoreTree()
  const names = computed(
    () => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name])),
  )
  const storeName = (id: string) => names.value.get(id) ?? ''

  const filter = computed<MineFilter>({
    get: () => (isMineFilter(route.query.type) ? route.query.type : 'all'),
    set: (value) => void router.replace({ query: { ...route.query, type: value === 'all' ? undefined : value } }),
  })
  const items = computed(() =>
    (list.data.value ?? []).filter((item) => filter.value === 'all' || item.kind === filter.value),
  )

  const toggle = useMutation({
    mutationFn: (input: SetMyAutomationActiveInput) => call<null>('set_my_automation_active', { input }),
    onMutate: (input) => {
      queryClient.setQueryData<MineItem[]>(MINE_KEY, (rows) =>
        rows?.map((row) => (row.kind === input.kind && row.id === input.id ? { ...row, active: input.active } : row)),
      )
    },
    onSettled: () => {
      for (const key of [WORKFLOW_KEYS.list, ['automations'], ['reminders'], ['price-alerts'], ['upcoming']]) {
        void queryClient.invalidateQueries({ queryKey: [...key] })
      }
    },
    meta: { error: 'automations.mine.toggleFailed', silent: 'the switch itself shows the new state' },
  })
  const setActive = (item: MineItem, active: boolean) => toggle.mutate({ kind: item.kind, id: item.id, active })

  return { list, items, filter, storeName, setActive }
}
