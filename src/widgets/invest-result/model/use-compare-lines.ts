import { computed, ref } from 'vue'
import { useQueries, useQuery } from '@tanstack/vue-query'

import { compareInput } from '@/features/invest-calculator'
import { call } from '@/shared/api/ipc'

export const MAX_COMPARED = 3
const VARIANTS = ['more_rate', 'more_contribution'] as const
type Variant = (typeof VARIANTS)[number]

/** One thing the run can be compared with: a «Что выгоднее» variant or a saved scenario. */
export interface CompareOption {
  key: string
  /** `variant`: from «Что выгоднее»; `goal`: a saved scenario a goal follows; `saved`: any other saved one. */
  tag: 'variant' | 'saved' | 'goal'
  name: string
  input: InvestInput
  /** What «щелчок по линии» opens: a variant goes into the form, a saved scenario is loaded. */
  open: { variant: Variant } | { scenario: string }
}

/** A picked option simulated: its balance at the end of each year and where it ends. */
export interface CompareLine {
  key: string
  name: string
  balances: number[]
  final: string
  open: CompareOption['open']
}

/**
 * «Сравнить с расчётами (до 3)»: the variants of the current run and the saved
 * scenarios; each picked one is simulated with its own parameters.
 * `name` gives a variant its label («Доходность 14 %», «Пополнения +20 %»).
 */
export function useCompareLines(
  currentId: () => string | null,
  current: () => InvestInput | null,
  name: (variant: Variant, input: InvestInput) => string,
) {
  const scenarios = useQuery({
    queryKey: ['calc-scenarios'],
    queryFn: () => call<CalcScenario[]>('calc_scenarios_list'),
  })
  const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })
  const picked = ref<string[]>([])

  const options = computed<CompareOption[]>(() => {
    const run = current()
    const variants: CompareOption[] = run
      ? VARIANTS.map((variant) => {
          const input = compareInput(run, variant)
          return { key: `variant:${variant}`, tag: 'variant', name: name(variant, input), input, open: { variant } }
        })
      : []
    const followed = new Set((goals.data.value ?? []).map((goal) => goal.scenario_id).filter(Boolean))
    const saved: CompareOption[] = (scenarios.data.value ?? [])
      .filter((scenario) => scenario.id !== currentId())
      .map((scenario) => ({
        key: `scenario:${scenario.id}`,
        tag: followed.has(scenario.id) ? 'goal' : 'saved',
        name: scenario.name,
        input: scenario.input,
        open: { scenario: scenario.id },
      }))
    return [...variants, ...saved]
  })
  const chosen = computed(() => options.value.filter((option) => picked.value.includes(option.key)))

  const runs = useQueries({
    queries: computed(() =>
      chosen.value.map((option) => ({
        queryKey: ['calc-invest', 'compare', option.input],
        queryFn: () => call<InvestResult>('calc_invest_simulate', { input: option.input }),
      })),
    ),
  })

  const lines = computed<CompareLine[]>(() =>
    chosen.value.flatMap((option, index) => {
      const result = runs.value[index]?.data
      if (!result) return []
      const balances = result.years.map((row) => Number(row.balance))
      return [{ key: option.key, name: option.name, balances, final: result.final_nominal, open: option.open }]
    }),
  )

  function toggle(key: string) {
    if (picked.value.includes(key)) picked.value = picked.value.filter((entry) => entry !== key)
    else if (picked.value.length < MAX_COMPARED) picked.value = [...picked.value, key]
  }

  return { options, picked, lines, toggle }
}

export type CompareLines = ReturnType<typeof useCompareLines>
