import type { QueryClient } from '@tanstack/vue-query'

import { CRYPTO_ACCOUNTS_KEY } from '@/entities/crypto-account'
import { CRYPTO_TOKENS_KEY } from '@/entities/crypto-token'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/**
 * A verdict on a token decides whether it counts at all: the module's lists and every figure read off the
 * holdings (wallet totals, the dashboard, planning) are refetched.
 */
export function refreshAfterTokenVerdict(queryClient: QueryClient) {
  for (const queryKey of [CRYPTO_ACCOUNTS_KEY, CRYPTO_TOKENS_KEY, ...LEDGER_KEYS, ...PLANNING_KEYS]) {
    void queryClient.invalidateQueries({ queryKey })
  }
}
