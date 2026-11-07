import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { call } from '@/shared/api/ipc'

export const SECURITY_STATE_KEY = ['security-state'] as const

/** Level, autolock and biometric state of the open vault (the settings card). */
export function useSecurityState() {
  return useQuery({
    queryKey: SECURITY_STATE_KEY,
    queryFn: () => call<SecurityState>('security_state'),
    placeholderData: keepPreviousData,
  })
}

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
    void queryClient.invalidateQueries({ queryKey: SECURITY_STATE_KEY })
    void queryClient.invalidateQueries({ queryKey: ['biometric-state'] })
  }
}

export function useBiometricDisable() {
  const refresh = useBiometricRefresh()
  return useMutation({
    mutationFn: () => call<BiometricState>('biometric_disable'),
    onSuccess: refresh,
    meta: { success: 'biometric.disabled', error: 'biometric.disableFailed' },
  })
}
