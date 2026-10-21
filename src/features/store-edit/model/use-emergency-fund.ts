import { computed, type MaybeRefOrGetter, toValue } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { textOf } from '@/shared/lib/notify'

export const EMERGENCY_FUND_KEY = ['emergency-fund']

/** Whether a wallet is part of the emergency fund, and the switch that adds or takes it out. */
export function useEmergencyFund(storeId: MaybeRefOrGetter<string>) {
  const queryClient = useQueryClient()
  const fund = useQuery({
    queryKey: EMERGENCY_FUND_KEY,
    queryFn: () => call<EmergencyStatus>('emergency_fund_get'),
  })
  const inFund = computed(() => fund.data.value?.fund.scope.store_ids.includes(toValue(storeId)) ?? false)
  const toggle = useMutation({
    mutationFn: (on: boolean) => call<EmergencyFund>('emergency_fund_toggle', { storeId: toValue(storeId), on }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: EMERGENCY_FUND_KEY }),
    meta: {
      error: 'emergency.saveFailed',
      success: (_data, on) => textOf((on as boolean) ? 'emergency.added' : 'emergency.removed'),
      undo: (_data, on) =>
        call<EmergencyFund>('emergency_fund_toggle', { storeId: toValue(storeId), on: !(on as boolean) }),
    },
  })
  return { fund, inFund, toggle }
}
