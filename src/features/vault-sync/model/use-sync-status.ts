import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

export const SYNC_STATUS_KEY = ['sync-status'] as const
export const SYNC_PEERS_KEY = ['sync-peers'] as const
export const SYNC_HISTORY_KEY = ['sync-history'] as const

/**
 * Sync of this vault with the other devices: the backend pushes every change
 * (`sync-status`, written into this query once by `useSyncEvents` in the
 * shell), so the query is only the first read and a fallback.
 */
export function useSyncStatus() {
  const status = useQuery({
    queryKey: SYNC_STATUS_KEY,
    queryFn: () => call<SyncStatus>('sync_status'),
  })

  const sessions = computed(() => status.data.value?.sessions ?? [])
  /** Devices with this vault, not paired yet: the banner offers them. */
  const offers = computed(() => sessions.value.filter((session) => session.phase === 'offer'))
  const pairing = computed(() => sessions.value.find((session) => session.phase === 'pairing' && session.pairing_code))
  const plan = computed(() => sessions.value.find((session) => session.phase === 'awaiting_confirm' && session.plan))
  const connected = computed(() => sessions.value.filter((session) => session.phase === 'live'))

  return { status, sessions, offers, pairing, plan, connected }
}
