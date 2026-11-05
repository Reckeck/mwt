// check-ui-ignore invalidate-all: a batch that moved the ledger changes what every screen derives
import { useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { useBackendEvent } from '@/shared/api/use-backend-event'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

import { syncErrorDescription } from '../lib/sync-error-text'
import { keysOfBatch, touchesSettings } from '../lib/sync-keys'

import { useSyncActions } from './use-sync-actions'
import { SYNC_HISTORY_KEY, SYNC_JOURNAL_KEY, SYNC_PEERS_KEY, SYNC_STATUS_KEY } from './use-sync-status'

/** The sessions a status names, to tell a new or gone device from a progress tick. */
const sessionSet = (status: SyncStatus | undefined) =>
  (status?.sessions ?? [])
    .map((session) => `${session.session_id}:${session.phase}`)
    .sort()
    .join('|')

/**
 * What sync tells in the background: a batch arrived (every screen re-reads,
 * a toast offers `Undo`), a session failed, the clocks disagree.
 * Mounted once by the shell.
 */
export function useSyncEvents() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settingsStore = useSettingsStore()
  // Called here, in setup: the composable injects the query client.
  const { undo } = useSyncActions()

  /** Re-read what a batch changed: the whole app only when the ledger moved. */
  function refreshAfter(summary: SyncSummary, collections: readonly string[] = []) {
    const keys = keysOfBatch(summary, collections)
    if (keys === 'all') void queryClient.invalidateQueries()
    else for (const queryKey of keys) void queryClient.invalidateQueries({ queryKey })
    if (touchesSettings(summary, collections)) void settingsStore.loadVault().catch(() => undefined)
  }

  // Pushed on every change, progress ticks included: the device list is re-read
  // only when a session comes, goes or changes phase.
  useBackendEvent<SyncStatus>('sync-status', (next) => {
    const before = sessionSet(queryClient.getQueryData<SyncStatus>(SYNC_STATUS_KEY))
    queryClient.setQueryData(SYNC_STATUS_KEY, next)
    if (sessionSet(next) !== before) void queryClient.invalidateQueries({ queryKey: SYNC_PEERS_KEY })
  })

  useBackendEvent<VaultSyncedEvent>('vault-synced', ({ batch, collections }) => {
    refreshAfter(batch.summary, collections)
    void queryClient.invalidateQueries({ queryKey: SYNC_JOURNAL_KEY })
    void queryClient.invalidateQueries({ queryKey: SYNC_HISTORY_KEY })
    void queryClient.invalidateQueries({ queryKey: SYNC_PEERS_KEY })
    // A stamp-only batch changed nothing the user could see or undo.
    if (batch.summary.total === 0) return
    notify.success(t('sync.synced', { name: batch.peer_name, count: batch.summary.total }), {
      action: { label: t('actions.undo'), onClick: () => undo.mutate(batch.id) },
    })
  })

  // A closed or locked session ends quietly; what arrives here is worth telling.
  useBackendEvent<SyncErrorEvent>('sync-error', (event) => {
    const error = { code: event.code, message: event.message }
    void queryClient.invalidateQueries({ queryKey: SYNC_JOURNAL_KEY })
    void queryClient.invalidateQueries({ queryKey: SYNC_PEERS_KEY })
    // Cut off on purpose or an incompatible version: the sentence names the device.
    const description = syncErrorDescription(event, t)
    notify.error(error, event.peer_name ? t('sync.failedWith', { name: event.peer_name }) : 'sync.failed', {
      description,
    })
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
