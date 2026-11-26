import { computed } from 'vue'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

const DATA_SOURCES_KEY = ['data-sources']

/** The user's data sources with the places that use them, and the two actions a row has: read now, accept or keep a new value. */
export function useSources() {
  const vault = useVaultStore()
  const queryClient = useQueryClient()
  const list = useQuery({
    queryKey: DATA_SOURCES_KEY,
    queryFn: () => call<DataSourceView[]>('datasources_list'),
    placeholderData: keepPreviousData,
    enabled: computed(() => !vault.locked),
  })
  const settle = () => {
    for (const queryKey of [DATA_SOURCES_KEY, ['inflation-overview']]) void queryClient.invalidateQueries({ queryKey })
  }
  const refresh = useMutation({
    mutationFn: (id: string) => call<DataSourceView>('datasource_refresh', { id }),
    onSuccess: settle,
    meta: { success: 'sources.refreshed', error: 'sources.refreshFailed' },
  })
  const confirm = useMutation({
    mutationFn: (input: { id: string; accept: boolean }) => call<DataSourceView>('datasource_confirm', input),
    onSuccess: settle,
    meta: { success: 'sources.confirmed', error: 'sources.saveFailed' },
  })
  return { list, refresh, confirm }
}
