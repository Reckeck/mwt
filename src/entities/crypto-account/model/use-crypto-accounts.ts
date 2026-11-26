import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { CRYPTO_ACCOUNTS_KEY } from './keys'

/** The accounts the module follows, with their wallets and what waits in quarantine. */
export function useCryptoAccounts() {
  return useQuery({
    queryKey: CRYPTO_ACCOUNTS_KEY,
    queryFn: () => call<CryptoAccountView[]>('crypto_list_accounts'),
    placeholderData: keepPreviousData,
  })
}

/** When each account was last read and whether the module may talk to the providers at all. */
export function useCryptoSyncStatus() {
  return useQuery({
    queryKey: [...CRYPTO_ACCOUNTS_KEY, 'sync'],
    queryFn: () => call<CryptoSyncStatus>('crypto_sync_status'),
    placeholderData: keepPreviousData,
  })
}
