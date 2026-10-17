import { useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { notify } from '@/shared/lib/notify'

import { SYNC_HISTORY_KEY } from './use-sync-status'

/**
 * What sync tells in the background: a batch arrived (every screen re-reads,
 * a toast offers «Отменить»), a session failed, the clocks disagree.
 * Mounted once by the shell.
 */
export function useSyncEvents() {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  useBackendEvent<VaultSyncedEvent>('vault-synced', ({ batch }) => {
    void queryClient.invalidateQueries()
    notify.success(t('sync.synced', { name: batch.peer_name, count: batch.summary.total }), {
      action: {
        label: t('actions.undo'),
        onClick: () => {
          call<UndoReport>('sync_undo', { batchId: batch.id })
            .then((report) => {
              notify.info(t('sync.undo.done', { count: report.restored }))
              void queryClient.invalidateQueries()
              void queryClient.invalidateQueries({ queryKey: SYNC_HISTORY_KEY })
            })
            .catch((err) => notify.error(err, 'sync.undo.failed'))
        },
      },
    })
  })

  // A closed or locked session ends quietly; what arrives here is worth telling.
  useBackendEvent<SyncErrorEvent>('sync-error', (event) => {
    const error = { code: event.code, message: event.message }
    notify.error(error, event.peer_name ? t('sync.failedWith', { name: event.peer_name }) : 'sync.failed')
  })

  useBackendEvent<SyncClockEvent>('sync-clock', (event) => {
    const minutes = Math.round(Math.abs(event.offset_ms) / 60_000)
    notify.warning(
      event.unreliable
        ? t('sync.clock.jumped', { name: event.peer_name })
        : t('sync.clock.skew', { name: event.peer_name, minutes }),
    )
  })
}
