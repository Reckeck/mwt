import { type Ref, computed } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { CRYPTO_TRANSITION_KEY } from '@/entities/crypto-account'
import { call } from '@/shared/api/ipc'

/**
 * What entering the module takes out of the totals (spec E11): the hand-kept crypto wallets that are
 * archived, and the capital in them. Read only, so it is asked while the wizard is open and nothing
 * else depends on it. A failed read is not an error of the wizard: the warning is simply absent.
 */
export function useTransitionPlan(enabled: Ref<boolean>) {
  const query = useQuery({
    queryKey: CRYPTO_TRANSITION_KEY,
    queryFn: () => call<CryptoTransitionPlan>('crypto_transition_plan'),
    enabled,
    placeholderData: keepPreviousData,
    retry: false,
  })
  return {
    plan: computed(() => (query.isError.value ? undefined : query.data.value)),
  }
}
