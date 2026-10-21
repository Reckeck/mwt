import { computed, ref } from 'vue'
import { useQueries, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import type { ScenarioLine } from './result-chart'

export const MAX_COMPARED = 3

/**
 * Up to three saved scenarios drawn over the current run: the list comes
 * from the vault, each picked one is simulated as it was saved.
 */
export function useScenarioLines(currentId: () => string | null) {
  const scenarios = useQuery({
    queryKey: ['calc-scenarios'],
    queryFn: () => call<CalcScenario[]>('calc_scenarios_list'),
  })
  const picked = ref<string[]>([])

  const others = computed(() => (scenarios.data.value ?? []).filter((entry) => entry.id !== currentId()))
  const chosen = computed(() => others.value.filter((entry) => picked.value.includes(entry.id)))

  const runs = useQueries({
    queries: computed(() =>
      chosen.value.map((scenario) => ({
        queryKey: ['calc-invest', 'scenario', scenario.id, scenario.updated_at],
        queryFn: () => call<InvestResult>('calc_invest_simulate', { input: scenario.input }),
      })),
    ),
  })

  const lines = computed<ScenarioLine[]>(() =>
    chosen.value.flatMap((scenario, index) => {
      const result = runs.value[index]?.data
      return result ? [{ name: scenario.name, balances: result.years.map((row) => Number(row.balance)) }] : []
    }),
  )

  function toggle(id: string) {
    if (picked.value.includes(id)) picked.value = picked.value.filter((entry) => entry !== id)
    else if (picked.value.length < MAX_COMPARED) picked.value = [...picked.value, id]
  }

  return { others, picked, lines, toggle }
}
