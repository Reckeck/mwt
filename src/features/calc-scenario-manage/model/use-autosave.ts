import { type ComputedRef, computed, ref, watch } from 'vue'
import { useDebounceFn } from '@vueuse/core'

import type { ScenarioBody, useScenarios } from './use-scenarios'

const AUTOSAVE_MS = 800

const sameBody = (scenario: CalcScenario, body: ScenarioBody) =>
  JSON.stringify(scenario.input) === JSON.stringify(body.input) &&
  JSON.stringify(scenario.source) === JSON.stringify(body.source) &&
  scenario.prefilled_at === body.prefilledAt

/**
 * A saved scenario keeps up with the form: 800 ms after the last change the
 * fields go into it. An unsaved run waits for «Сохранить» and a name.
 */
export function useAutosave(
  current: ComputedRef<CalcScenario | null>,
  snapshot: () => ScenarioBody,
  autosave: ReturnType<typeof useScenarios>['autosave'],
) {
  const body = computed(() => (current.value ? snapshot() : null))
  const changed = computed(() => Boolean(current.value && body.value && !sameBody(current.value, body.value)))

  // Core may write the amounts back differently («10000» → «10000.00»); what was sent is not sent again.
  const sent = ref<string | null>(null)
  const flush = useDebounceFn(() => {
    const scenario = current.value
    if (!scenario || !body.value || !changed.value || autosave.isPending.value) return
    const key = JSON.stringify([scenario.id, body.value])
    if (key === sent.value) return
    sent.value = key
    autosave.mutate({ id: scenario.id, name: scenario.name, body: body.value })
  }, AUTOSAVE_MS)

  watch(body, () => {
    if (changed.value) void flush()
  })

  return { saving: computed(() => autosave.isPending.value) }
}
