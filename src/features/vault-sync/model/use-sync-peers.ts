import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { SYNC_PEERS_KEY } from './use-sync-status'

/** The trusted devices (paired with this vault; `forgotten` lists the ones the user forgot) and the time of the latest sync with any of them. */
export function useSyncPeers() {
  const peers = useQuery({ queryKey: SYNC_PEERS_KEY, queryFn: () => call<SyncPeerView[]>('sync_peers') })
  const all = computed(() => peers.data.value ?? [])
  // Forgotten devices are kept apart: they are refused until "Pair again".
  const list = computed(() => all.value.filter((peer) => !peer.forgotten))
  const forgotten = computed(() => all.value.filter((peer) => peer.forgotten))
  return { peers, list, forgotten }
}
