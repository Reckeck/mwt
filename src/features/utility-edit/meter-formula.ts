import { type Ref, computed } from 'vue'
import { refDebounced } from '@vueuse/core'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** How a meter is priced: consumption × tariff, or its own formula. */
export type MeterMode = 'tariff' | 'formula'

/** The names a formula reads, Russian and Latin (`utilities_formula::VARIABLES`). */
export const FORMULA_VARIABLES = [
  ['показание', 'current'],
  ['предыдущее', 'previous'],
  ['расход', 'used'],
  ['тариф', 'tariff'],
  ['дней', 'days'],
] as const

/** A zone's name inside a variable: «пик ночь» → «пик_ночь». */
export const zoneKey = (zone: string) => zone.trim().toLowerCase().split(/\s+/).join('_')

/**
 * The bill by the formula being typed, and what is wrong with it: on the last
 * real readings when the meter has them (`sample`), else on 100 units per zone.
 */
export function useFormulaPreview(state: {
  tariff: Ref<string>
  mode: Ref<MeterMode>
  formula: Ref<string>
  zones: Ref<string[]>
  sample: Ref<{ previous: string; current: string }[] | null>
}) {
  const formula = refDebounced(state.formula, 400)
  const input = computed<UtilityFormulaInput>(() => {
    const zones = state.zones.value
    const sample = state.sample.value
    const fields = zones.length ? zones : [null]
    return {
      tariff: state.tariff.value.trim() || '0',
      formula: state.mode.value === 'formula' ? formula.value : null,
      zones,
      readings: fields.map((zone, index) => ({
        zone,
        previous: sample?.length === fields.length ? sample[index].previous : '0',
        current: sample?.length === fields.length ? sample[index].current : '100',
      })),
      days: null,
    }
  })
  const query = useQuery({
    queryKey: computed(() => ['utility-formula-preview', input.value]),
    queryFn: () => call<UtilityFormulaPreview>('preview_utility_formula', { input: input.value }),
    placeholderData: keepPreviousData,
  })
  return computed(() => (query.data.value ? { ...query.data.value, input: input.value } : null))
}
