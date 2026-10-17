import { computed } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'

export const SYNC_STATUS_KEY = ['sync-status'] as const
export const SYNC_PEERS_KEY = ['sync-peers'] as const
export const SYNC_HISTORY_KEY = ['sync-history'] as const

/**
 * Sync of this vault with the other devices: the backend pushes every change
 * (`sync-status`), so the query is only the first read and a fallback.
 */
export function useSyncStatus() {
  const queryClient = useQueryClient()
  const status = useQuery({
    queryKey: SYNC_STATUS_KEY,
    queryFn: () => call<SyncStatus>('sync_status'),
  })
  useBackendEvent<SyncStatus>('sync-status', (next) => {
    queryClient.setQueryData(SYNC_STATUS_KEY, next)
    void queryClient.invalidateQueries({ queryKey: SYNC_PEERS_KEY })
  })

  const sessions = computed(() => status.data.value?.sessions ?? [])
  /** Devices with this vault, not paired yet: the banner offers them. */
  const offers = computed(() => sessions.value.filter((session) => session.phase === 'offer'))
  const pairing = computed(() => sessions.value.find((session) => session.phase === 'pairing' && session.pairing_code))
  const plan = computed(() => sessions.value.find((session) => session.phase === 'awaiting_confirm' && session.plan))
  const connected = computed(() => sessions.value.filter((session) => session.phase === 'live'))

  return { status, sessions, offers, pairing, plan, connected }
}
