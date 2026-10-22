import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { CALC_SCENARIOS_KEY } from '@/shared/api/queries'

export const SCENARIOS_KEY = CALC_SCENARIOS_KEY

/** What is saved: the fields as the engine reads them and where «Из данных» took them from. */
export interface ScenarioBody {
  input: InvestInput
  source: ScenarioSource | null
  prefilledAt: string | null
}

/** Saved calculator runs and what the user does with them. */
export function useScenarios() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const list = useQuery({ queryKey: SCENARIOS_KEY, queryFn: () => call<CalcScenario[]>('calc_scenarios_list') })
  const byId = (id: string | null) => list.data.value?.find((entry) => entry.id === id) ?? null
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: SCENARIOS_KEY })
    void queryClient.invalidateQueries({ queryKey: ['goal-progress'] })
  }

  const save = useMutation({
    mutationFn: (draft: { id: string | null; name: string; body: ScenarioBody }) =>
      call<CalcScenario>('calc_scenario_save', {
        draft: {
          id: draft.id,
          name: draft.name,
          input: draft.body.input,
          source: draft.body.source,
          prefilled_at: draft.body.prefilledAt,
        },
      }),
    onSuccess: refresh,
    meta: {
      error: 'invest.scenario.saveFailed',
      success: (data) => ({ text: t('invest.scenario.saved'), description: (data as CalcScenario).name }),
    },
  })

  const remove = useMutation({
    mutationFn: (id: string) => call<CalcScenarioDeleted>('calc_scenario_delete', { id }),
    onSuccess: refresh,
    meta: {
      error: 'invest.scenario.deleteFailed',
      success: (data) => ({
        text: t('invest.scenario.deleted'),
        description: (data as CalcScenarioDeleted).scenario.name,
      }),
      undo: (data) => call<CalcScenario>('calc_scenario_restore', { deleted: data as CalcScenarioDeleted }),
    },
  })

  const goalDraft = useMutation({
    mutationFn: (id: string) => call<GoalInput>('goal_from_scenario', { id }),
    meta: { error: 'invest.scenario.goalFailed', silent: 'opens the goal form, which reports the save itself' },
  })

  const scenarios = computed(() => list.data.value ?? [])
  return { list, scenarios, byId, save, remove, goalDraft }
}
