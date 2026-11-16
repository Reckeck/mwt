import { type MaybeRefOrGetter, computed, toValue } from 'vue'
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

/** The account a wallet shows (`Store.crypto_account_id`), once the list is there. */
export function useCryptoAccountOf(accountId: MaybeRefOrGetter<string | null | undefined>) {
  const accounts = useCryptoAccounts()
  const account = computed(() => {
    const id = toValue(accountId)
    return id ? (accounts.data.value ?? []).find((entry) => entry.id === id) : undefined
  })
  return { accounts, account }
}

/** When each account was last read and whether the module may talk to the providers at all. */
export function useCryptoSyncStatus() {
  return useQuery({
    queryKey: [...CRYPTO_ACCOUNTS_KEY, 'sync'],
    queryFn: () => call<CryptoSyncStatus>('crypto_sync_status'),
    placeholderData: keepPreviousData,
  })
}
