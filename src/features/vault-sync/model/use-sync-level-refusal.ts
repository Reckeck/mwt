import { reactive } from 'vue'

/**
 * The `SECURITY_LEVEL_REQUIRED` refusals of this session. Both sides of a refused exchange get the same
 * event: the device below level 3 (`here`: it did not take the crypto data) and the one that sent it
 * (`peers`: whose names refused). Kept for the unlocked session only: `useSyncEvents` resets it on lock,
 * `here` also stops showing once the vault reaches level 3, a peer is dropped when a batch comes from it.
 */
const refusal = reactive({ here: false, peers: [] as string[] })

export function useSyncLevelRefusal() {
  return {
    refusal,
    /** This device refused crypto data: it is below level 3. */
    refusedHere() {
      refusal.here = true
    },
    /** `name` is below level 3 and did not take the crypto data this device sent. */
    refusedBy(name: string) {
      if (!refusal.peers.includes(name)) refusal.peers.push(name)
    },
    /** `name` sent a batch: it is at level 3 now (or the data went through). */
    accepted(name: string) {
      const index = refusal.peers.indexOf(name)
      if (index >= 0) refusal.peers.splice(index, 1)
    },
    reset() {
      refusal.here = false
      refusal.peers.splice(0)
    },
  }
}
