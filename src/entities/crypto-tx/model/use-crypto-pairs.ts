import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { CRYPTO_OPERATIONS_KEY } from './keys'

/**
 * The pointers of one operation or of one fiat record (spec E13), both ends resolved. Without crypto data the
 * answer is empty, so a fiat screen shows nothing. Under `CRYPTO_OPERATIONS_KEY`, so a link or an unlink
 * refreshes the list, the card and this together.
 */
export function useCryptoPairs(
  input: MaybeRefOrGetter<CryptoPairsInput | null>,
  options: { enabled?: MaybeRefOrGetter<boolean> } = {},
) {
  return useQuery({
    queryKey: computed(() => [...CRYPTO_OPERATIONS_KEY, 'pairs', toValue(input)]),
    queryFn: () => call<CryptoPair[]>('crypto_link_pairs', { input: toValue(input) }),
    enabled: computed(() => toValue(input) !== null && (toValue(options.enabled) ?? true)),
    placeholderData: keepPreviousData,
  })
}
