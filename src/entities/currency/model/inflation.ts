import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

export const INFLATION_OVERVIEW_KEY = ['inflation-overview'] as const

/**
 * Each currency's inflation as the vault knows it: the fetched monthly
 * series (Auto) or the user's flat rate (Manual). Locked, only the flat
 * rates from the app settings are known.
 */
export function useInflationOverview() {
  const vault = useVaultStore()
  const settings = useSettingsStore()
  const query = useQuery({
    queryKey: INFLATION_OVERVIEW_KEY,
    queryFn: () => call<InflationCurrency[]>('get_inflation_overview'),
    enabled: computed(() => !vault.locked),
  })

  /** What reports and the calculator use for `code` now, percent per year. */
  const effectiveOf = (code: string): string => {
    const lower = code.toLowerCase()
    const row = query.data.value?.find((entry) => entry.currency === lower)
    return row?.effective ?? settings.app?.inflation?.[lower] ?? '0'
  }

  return { query, effectiveOf }
}
