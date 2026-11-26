import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api'

export const CRYPTO_POSITIONS_KEY = ['crypto', 'positions']

/** The DeFi positions of every followed address (`crypto_list_positions`). */
export const useCryptoPositions = () =>
  useQuery({
    queryKey: CRYPTO_POSITIONS_KEY,
    queryFn: () => call<CryptoPositionView[]>('crypto_list_positions'),
    placeholderData: keepPreviousData,
  })
