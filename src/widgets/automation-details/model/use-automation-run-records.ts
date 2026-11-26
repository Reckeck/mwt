import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

import { recordsById } from '../lib/run-records'

/** The run records of a repeat, by run id; off for a scheme (`id` is `null`), which has none. */
export function useAutomationRunRecords(id: MaybeRefOrGetter<string | null>) {
  const records = useQuery({
    // no placeholder: another automation's runs; under `workflows`, so a retry or a skip refreshes it
    queryKey: computed(() => [...WORKFLOW_KEYS.list, 'run-records', toValue(id)]),
    queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: toValue(id) }),
    enabled: computed(() => toValue(id) !== null),
  })
  return computed(() => recordsById(records.data.value))
}
