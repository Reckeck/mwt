import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** How far ahead the next run is looked for: a monthly one always shows. */
const AHEAD_DAYS = 62

/**
 * What booked an operation (Paths 2 · 10): the automation, the run that made
 * this very record and the next planned one. Read only while the card is open.
 */
export function useAutomationGlance(
  automationId: MaybeRefOrGetter<string>,
  runId: MaybeRefOrGetter<string | null>,
  enabled: MaybeRefOrGetter<boolean>,
) {
  const on = computed(() => toValue(enabled))
  const automations = useQuery({
    queryKey: ['automations'],
    queryFn: () => call<Automation[]>('list_automations'),
    enabled: on,
  })
  const runs = useQuery({
    // no placeholder: another automation's runs
    queryKey: computed(() => ['automation-runs', toValue(automationId)]),
    queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: toValue(automationId) }),
    enabled: on,
  })
  const upcoming = useQuery({
    queryKey: ['upcoming', AHEAD_DAYS],
    queryFn: () => call<UpcomingItem[]>('get_upcoming', { days: AHEAD_DAYS }),
    enabled: on,
  })

  const automation = computed(() => automations.data.value?.find((item) => item.id === toValue(automationId)) ?? null)
  const run = computed(() => runs.data.value?.find((item) => item.id === toValue(runId)) ?? null)
  const next = computed(
    () =>
      upcoming.data.value?.find((item) => item.kind === 'automation' && item.source_id === toValue(automationId)) ??
      null,
  )
  return { automation, run, next, loading: computed(() => automations.isPending.value) }
}
