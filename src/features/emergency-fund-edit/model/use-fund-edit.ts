import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY, EMERGENCY_FUND_KEY } from '@/shared/api/queries'
import { textOf } from '@/shared/lib/notify'

interface FundChange {
  fund: EmergencyFund
  /** The fund before the change, for «Отменить». */
  previous: EmergencyFund
}

function useRefresh() {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({ queryKey: EMERGENCY_FUND_KEY })
    void queryClient.invalidateQueries({ queryKey: CAPITAL_HEALTH_KEY })
  }
}

/** Saves the whole fund (accounts and months); an empty list switches the fund off. */
export function useFundSet() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: ({ fund }: FundChange) => call<EmergencyFund>('emergency_fund_set', { fund }),
    onSuccess: refresh,
    meta: {
      error: 'emergency.saveFailed',
      success: () => textOf('emergency.saved'),
      undo: (_data, change) => call<EmergencyFund>('emergency_fund_set', { fund: (change as FundChange).previous }),
    },
  })
}

/** «Сделать подушкой» on a candidate account of the checkup. */
export function useFundAdd() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: (storeId: string) => call<EmergencyFund>('emergency_fund_toggle', { storeId, on: true }),
    onSuccess: refresh,
    meta: {
      error: 'emergency.saveFailed',
      success: () => textOf('emergency.added'),
      undo: (_data, storeId) => call<EmergencyFund>('emergency_fund_toggle', { storeId: storeId as string, on: false }),
    },
  })
}
