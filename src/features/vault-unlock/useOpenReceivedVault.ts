import { useQueryClient } from '@tanstack/vue-query'
import { useRouter } from 'vue-router'

import { useVaultStore } from '@/shared/store/vault'

import { useUnlockVault } from './useUnlockVault'

/**
 * Opening a vault that just arrived from another device: one without a password opens
 * at once, an encrypted one is left to the unlock dialog (nothing is typed twice).
 */
export function useOpenReceivedVault() {
  const vault = useVaultStore()
  const router = useRouter()
  const queryClient = useQueryClient()
  const unlocking = useUnlockVault()

  /** `true`: the vault is open. `false`: the password (or the base currency) is still to be asked. */
  async function openIfPlain(path: string): Promise<boolean> {
    // The recents must be fresh: the received file is not in the list loaded before.
    await vault.loadRecents().catch(() => {})
    unlocking.reset(path)
    const plain = vault.recents.find((recent: RecentVault) => recent.path === path)?.encrypted === false
    if (!plain || unlocking.baseCurrency.asked.value) return false
    await unlocking.submit()
    if (unlocking.error.value !== null || unlocking.conflict.value) return false
    // Another vault: nothing cached belongs to it.
    queryClient.clear()
    return true
  }

  /**
   * From the settings, with another vault open: a plain received vault replaces it in place;
   * otherwise the open one is locked and the unlock screen asks for the password.
   */
  async function switchTo(path: string): Promise<void> {
    if (await openIfPlain(path)) return
    await vault.lock()
    await router.replace({ name: 'unlock', query: { received: path } })
  }

  return { openIfPlain, switchTo }
}
