import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { recordsById } from '../lib/run-records'

/** Root of the run-records cache; one of `PLANNING_KEYS`, which every run action invalidates. */
export const AUTOMATION_RUNS_KEY = ['automation-runs'] as const

/** The run records of a repeat, by run id; off for a scheme (`id` is `null`), which has none. */
export function useAutomationRunRecords(id: MaybeRefOrGetter<string | null>) {
  const records = useQuery({
    // no placeholder: another automation's runs; under `automation-runs` (a PLANNING_KEYS entry, shared with the glance), so a retry or a skip refreshes it
    queryKey: computed(() => [...AUTOMATION_RUNS_KEY, toValue(id)]),
    queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: toValue(id) }),
    enabled: computed(() => toValue(id) !== null),
  })
  return computed(() => recordsById(records.data.value))
}
