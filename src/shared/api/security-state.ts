import { useQuery } from '@tanstack/vue-query'

import { call } from './ipc'

export const SECURITY_STATE_QUERY_KEY = 'security-state'

/** The vault's protection level, the autolock and the device keys: one answer for the levels card and the wizard. */
export function useSecurityState() {
  return useQuery({
    queryKey: [SECURITY_STATE_QUERY_KEY],
    queryFn: () => call<SecurityState>('security_state'),
  })
}
