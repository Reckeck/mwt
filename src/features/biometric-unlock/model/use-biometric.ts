import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { call } from '@/shared/api/ipc'
import { SECURITY_STATE_QUERY_KEY } from '@/shared/api/security-state'

/** Biometric state of the vault file the lock screen shows; not open yet, so by path. */
export function useBiometricState(path: MaybeRefOrGetter<string>) {
  return useQuery({
    queryKey: computed(() => ['biometric-state', toValue(path)] as const),
    queryFn: () => call<BiometricState>('biometric_state', { input: { path: toValue(path) } }),
    placeholderData: keepPreviousData,
  })
}

/** After any change of the binding both views are read again. */
export function useBiometricRefresh() {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({ queryKey: [SECURITY_STATE_QUERY_KEY] })
    void queryClient.invalidateQueries({ queryKey: ['biometric-state'] })
  }
}
