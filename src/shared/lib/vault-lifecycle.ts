/** One "vault closed" hook: whatever holds decrypted vault data subscribes here and drops it. */
export type VaultClosedReason = 'locked' | 'switched' | 'sandbox-exit'

type Callback = (reason: VaultClosedReason) => void

const subscribers = new Set<Callback>()

/** Returns the unsubscribe function. */
export function onVaultClosed(callback: Callback): () => void {
  subscribers.add(callback)
  return () => {
    subscribers.delete(callback)
  }
}

export function emitVaultClosed(reason: VaultClosedReason): void {
  for (const callback of [...subscribers]) callback(reason)
}
