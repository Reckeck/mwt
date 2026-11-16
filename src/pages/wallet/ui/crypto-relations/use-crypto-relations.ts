import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { CRYPTO_OPERATIONS_KEY } from '@/entities/crypto-tx'
import { call } from '@/shared/api/ipc'

/**
 * Who a followed account dealt with (spec E15), read from its events on every call. Under
 * `CRYPTO_OPERATIONS_KEY`, so a correction or a link refreshes it together with the list.
 */
export function useCryptoRelations(accountId: MaybeRefOrGetter<string>) {
  return useQuery({
    queryKey: computed(() => [...CRYPTO_OPERATIONS_KEY, 'relations', toValue(accountId)]),
    queryFn: () => call<CryptoRelations>('crypto_relations', { input: { account_id: toValue(accountId) } }),
    placeholderData: keepPreviousData,
  })
}
