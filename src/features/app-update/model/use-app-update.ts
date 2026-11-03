import { computed } from 'vue'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { useSettingsStore } from '@/shared/store/settings'

import { describeUpdate } from '../lib/update-view'
import type { UpdateAction } from '../lib/update-view'

export const UPDATE_STATUS_KEY = ['update-status'] as const

/**
 * The state of the app update. The backend pushes every change (`update-state`,
 * written into the query by `useUpdateEvents` in the shell), so the query is the
 * first read and a fallback.
 */
export function useAppUpdate() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settingsStore = useSettingsStore()

  const status = useQuery({
    queryKey: UPDATE_STATUS_KEY,
    queryFn: () => call<UpdateStatusDto>('update_status'),
    placeholderData: keepPreviousData,
  })
  const view = computed(() => describeUpdate(status.data.value))

  const checkNow = useMutation({
    mutationFn: () => call<UpdateStatusDto>('update_check_now'),
    onSuccess: (next) => queryClient.setQueryData(UPDATE_STATUS_KEY, next),
    meta: {
      success: (next) => {
        const kind = describeUpdate(next as UpdateStatusDto).kind
        if (kind === 'error') return t('update.notice.checkFailed')
        if (kind === 'upToDate') return t('update.notice.upToDate')
        return t('update.notice.found')
      },
      error: 'update.notice.checkFailed',
    },
  })

  const restart = useMutation({
    mutationFn: () => call<null>('update_restart_now'),
    meta: { silent: 'the app restarts; a refusal is shown by the status line' },
  })

  const autoUpdate = computed(() => settingsStore.app?.auto_update ?? true)
  const setAuto = useMutation({
    mutationFn: (enabled: boolean) => settingsStore.updateApp({ auto_update: enabled }),
    onSettled: () => void queryClient.invalidateQueries({ queryKey: UPDATE_STATUS_KEY }),
    meta: { success: 'settings.saved', error: 'settings.saveFailed' },
  })

  /** Run the action of a view: the restart, or open its page in the browser. */
  function run(action: UpdateAction, url: string | null) {
    if (action === 'restart') restart.mutate()
    else if ((action === 'openRelease' || action === 'openApk') && url) void call('open_explorer', { url })
  }

  return { status, view, checkNow, restart, autoUpdate, setAuto, run }
}

/** Mounted once by the shell: `update-state` goes straight into the query. */
export function useUpdateEvents() {
  const queryClient = useQueryClient()
  useBackendEvent<UpdateStatusDto>('update-state', (next) => queryClient.setQueryData(UPDATE_STATUS_KEY, next))
}
