import { queryClient } from '@/app/providers/query'
import { onVaultClosed } from '@/shared/lib/vault-lifecycle'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

/**
 * Everything that holds decrypted vault data drops it here, once the vault is locked,
 * replaced or left (the one place that clears the query cache).
 * Call after Pinia is installed; returns the unsubscribe function.
 */
export function registerVaultClosedSubscribers(): () => void {
  return onVaultClosed((reason) => {
    queryClient.clear()
    const settings = useSettingsStore()
    settings.forgetVault()
    resetTradeLookup()
    // A replaced vault is already open: its settings are loaded again (a lock reloads on unlock).
    if (reason === 'switched') settings.loadVault().catch(() => {})
  })
}

function resetTradeLookup(): void {
  const lookup = useTradeLookupStore()
  lookup.reset()
  lookup.executions = []
  lookup.keyNotice = null
  lookup.key = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }
}
