import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { DATA_SOURCES_KEY } from '@/entities/data-source'
import { call } from '@/shared/api/ipc'

/** What follows a source's value: the list itself and the inflation table that may read it. */
const FOLLOWS_SOURCES = [DATA_SOURCES_KEY, ['inflation-overview']] as const

/** Save, delete, refresh and confirm a data source; the page preview and `Test` of the form. */
export function useSourceActions() {
  const queryClient = useQueryClient()
  const settle = () => {
    for (const queryKey of FOLLOWS_SOURCES) void queryClient.invalidateQueries({ queryKey: [...queryKey] })
  }

  const preview = useMutation({
    mutationFn: (url: string) => call<SourcePreview>('datasource_preview', { url }),
    meta: { silent: 'the frame shows the page; a failure is shown under the address', error: 'sources.previewFailed' },
  })

  const test = useMutation({
    mutationFn: (draft: DataSourceDraft) => call<Extracted>('datasource_test', { draft }),
    meta: { silent: 'the value is shown next to «Проверить»' },
  })

  const save = useMutation({
    mutationFn: (draft: DataSourceDraft) => call<DataSourceView>('datasource_save', { draft }),
    onSuccess: settle,
    meta: { success: 'sources.saved', error: 'sources.saveFailed' },
  })

  const remove = useMutation({
    mutationFn: (id: string) => call<void>('datasource_delete', { id }),
    onSuccess: settle,
    meta: { success: 'sources.deleted', error: 'sources.deleteFailed' },
  })

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

  return { preview, test, save, remove, refresh, confirm }
}
