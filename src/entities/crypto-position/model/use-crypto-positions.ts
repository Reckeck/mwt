import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { CRYPTO_POSITIONS_KEY } from './keys'

/** Every DeFi position of every followed address, open and closed: the list filters, the engine computes. */
export function useCryptoPositions() {
  return useQuery({
    queryKey: [...CRYPTO_POSITIONS_KEY, 'list'],
    queryFn: () => call<CryptoPositionView[]>('crypto_list_positions'),
    placeholderData: keepPreviousData,
  })
}
