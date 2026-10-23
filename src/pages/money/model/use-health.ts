import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY } from '@/shared/api/queries'

/** The checkup report; the folded full checkup below reads the same cache. */
export function useHealth() {
  return useQuery({
    queryKey: CAPITAL_HEALTH_KEY,
    queryFn: () => call<HealthReport>('capital_health'),
    placeholderData: keepPreviousData,
  })
}
