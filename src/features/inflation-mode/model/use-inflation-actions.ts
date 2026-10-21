import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { INFLATION_OVERVIEW_KEY } from '@/entities/currency'
import { call } from '@/shared/api/ipc'

/** Keys whose figures follow a currency's inflation (the overview itself is set from the answer). */
const FOLLOWS_INFLATION = [['report'], ['interest-tax']] as const

/**
 * «Обновить» (inflation and the key rate from the statistics offices now)
 * and the Auto / Manual choice of a currency.
 */
export function useInflationActions() {
  const queryClient = useQueryClient()
  const settle = (overview: InflationCurrency[]) => {
    queryClient.setQueryData(INFLATION_OVERVIEW_KEY, overview)
    for (const queryKey of FOLLOWS_INFLATION) void queryClient.invalidateQueries({ queryKey })
  }

  const refresh = useMutation({
    mutationFn: () => call<InflationCurrency[]>('refresh_rates'),
    onSuccess: settle,
    meta: { success: 'inflation.refreshed', error: 'inflation.refreshFailed' },
  })

  const setMode = useMutation({
    mutationFn: (input: { currency: string; mode: InflationMode | null }) =>
      call<InflationCurrency[]>('set_inflation_mode', input),
    onSuccess: settle,
    meta: { success: 'inflation.modeSaved', error: 'settings.saveFailed' },
  })

  return { refresh, setMode }
}
