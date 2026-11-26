import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

/**
 * Currencies the shipped flat table covers (`default_inflation` in the core):
 * their row cannot be removed, clearing it would only bring the shipped rate back.
 */
export const SHIPPED_INFLATION_CODES: readonly string[] = ['rub', 'usd', 'eur', 'thb']

export const INFLATION_OVERVIEW_KEY = ['inflation-overview'] as const

/**
 * Each currency's inflation as the vault knows it: the fetched monthly
 * series (Auto) or the group's flat rate (Manual). Locked, nothing is known.
 */
export function useInflationOverview() {
  const vault = useVaultStore()
  const query = useQuery({
    queryKey: INFLATION_OVERVIEW_KEY,
    queryFn: () => call<InflationCurrency[]>('get_inflation_overview'),
    enabled: computed(() => !vault.locked),
  })

  /** What reports and the calculator use for `code` now, percent per year. */
  const effectiveOf = (code: string): string => {
    const lower = code.toLowerCase()
    const row = query.data.value?.find((entry) => entry.currency === lower)
    return row?.effective ?? '0'
  }

  return { query, effectiveOf }
}
