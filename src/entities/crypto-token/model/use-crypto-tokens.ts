import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { CRYPTO_TOKENS_KEY } from './keys'

/** The tokens the module has met, with their status and what is suspicious about their text. */
export function useCryptoTokens() {
  return useQuery({
    queryKey: CRYPTO_TOKENS_KEY,
    queryFn: () => call<TokenView[]>('crypto_list_tokens'),
    placeholderData: keepPreviousData,
  })
}
