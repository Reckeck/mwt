import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Query key of `list_my_automations`: under `workflows`, so a write there refreshes it. */
export const MINE_KEY = [...WORKFLOW_KEYS.list, 'mine']

export type MineFilter = 'all' | MineKind
const FILTERS: MineFilter[] = ['all', 'repeat', 'scheme', 'reminder', 'alert']
export const MINE_FILTERS = FILTERS

/** Everything that fires on its own: the list, the chip that narrows it (`?type=`), the switch of a row. */
export function useMine() {
  const route = useRoute()
  const router = useRouter()
  const queryClient = useQueryClient()

  const list = useQuery({
    queryKey: MINE_KEY,
    queryFn: () => call<MineItem[]>('list_my_automations'),
    placeholderData: keepPreviousData,
  })

  const filter = computed<MineFilter>({
    get: () => (FILTERS.includes(route.query.type as MineFilter) ? (route.query.type as MineFilter) : 'all'),
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

  return { list, items, filter, setActive }
}
