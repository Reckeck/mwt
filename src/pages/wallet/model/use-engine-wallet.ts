import { type Ref, computed } from 'vue'

import { useCryptoAccountOf, useCryptoSyncStatus } from '@/entities/crypto-account'

import { isEngineDriven } from '../lib/engine-wallet'

/**
 * A wallet driven by the crypto engine (a connected address, an exchange read through its key): the
 * followed account it shows and when that was last read. Such a wallet only reads, so the page
 * switches off everything that would create or edit an operation by hand.
 */
export function useEngineWallet(store: Ref<Store | undefined | null>) {
  const engine = computed(() => isEngineDriven(store.value))
  const { account } = useCryptoAccountOf(() => store.value?.crypto_account_id)
  const sync = useCryptoSyncStatus()
  const readAt = computed(
    () => sync.data.value?.accounts.find((entry) => entry.account_id === account.value?.id)?.last_ok_at ?? null,
  )
  return { engine, account, readAt }
}
