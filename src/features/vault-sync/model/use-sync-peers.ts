import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { SYNC_PEERS_KEY } from './use-sync-status'

/** The trusted devices (paired with this vault) and the time of the latest sync with any of them. */
export function useSyncPeers() {
  const peers = useQuery({ queryKey: SYNC_PEERS_KEY, queryFn: () => call<SyncPeerView[]>('sync_peers') })
  const list = computed(() => peers.data.value ?? [])
  return { peers, list }
}
