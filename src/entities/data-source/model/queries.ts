import { computed, toValue } from 'vue'
import type { MaybeRefOrGetter } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

export const DATA_SOURCES_KEY = ['data-sources'] as const

/** The user's own data sources with the places that use them (plan I2). */
export function useDataSources(enabled: MaybeRefOrGetter<boolean> = true) {
  const vault = useVaultStore()
  return useQuery({
    queryKey: DATA_SOURCES_KEY,
    queryFn: () => call<DataSourceView[]>('datasources_list'),
    placeholderData: keepPreviousData,
    enabled: computed(() => !vault.locked && toValue(enabled)),
  })
}

/** The value a source holds now, as text: a number, the last point of a series or the text. */
export function sourceValueText(value: SourceValue | null): string | null {
  if (!value) return null
  if (value.number !== null) return value.number
  const last = value.series.at(-1)
  if (last) return last.value
  return value.text
}
