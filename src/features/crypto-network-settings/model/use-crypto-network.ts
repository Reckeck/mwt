import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { CRYPTO_ACCOUNTS_KEY } from '@/entities/crypto-account'
import { call, parseError } from '@/shared/api/ipc'

/** Everything the crypto settings read lives under this key, so one invalidation reaches all of it. */
export const CRYPTO_NETWORK_KEY = ['crypto', 'network-settings'] as const
/** The sync status is one cache entry for the whole module: the connect dialog and Settings read the same one. */
export const CRYPTO_SYNC_STATUS_KEY = [...CRYPTO_ACCOUNTS_KEY, 'sync'] as const

export function useNetworkSettings() {
  return useQuery({
    queryKey: CRYPTO_NETWORK_KEY,
    queryFn: () => call<NetworkSettings>('crypto_network_settings'),
    placeholderData: keepPreviousData,
  })
}

/** The one-time consent to talk to the providers: `null` until asked. The status is E6's, the consent a field of it. */
export function useTrackingConsent() {
  return useQuery({
    queryKey: CRYPTO_SYNC_STATUS_KEY,
    queryFn: () => call<CryptoSyncStatus>('crypto_sync_status'),
    select: (status) => status.consent ?? null,
    placeholderData: keepPreviousData,
  })
}

export function useSaveNetwork() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: SetNetworkSettingsInput) => call('crypto_set_network_settings', { input }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: CRYPTO_NETWORK_KEY }),
    meta: { success: 'cryptoTrack.network.saved', error: 'cryptoTrack.network.saveFailed' },
  })
}

/** What a test is called with: the request and the network's name for the notice. */
export interface TestVariables {
  input: TestNetworkInput
  label: string
}

export interface NetworkTestOutcomeNote {
  chain: string
  result: NetworkTestResult
}

/**
 * The connection test. A node that did not answer is an error toast, so the outcome reads the same in the list and
 * in the notice; the result itself goes to `onResult` for the row's line. The text comes from here, never from the
 * provider, so nothing a node says reaches the screen.
 */
export function useTestNetwork(onResult: (note: NetworkTestOutcomeNote) => void) {
  const { t, n } = useI18n()
  return useMutation({
    mutationFn: async ({ input }: TestVariables) => {
      const result = await call<NetworkTestResult>('crypto_test_network', { input })
      onResult({ chain: input.chain, result })
      return result
    },
    meta: {
      success: (data: unknown, variables: unknown) => {
        const result = data as NetworkTestResult
        const { label: name } = variables as TestVariables
        if (result.outcome === 'ok') {
          return result.block === undefined
            ? t('cryptoTrack.network.testOkScout', { name, ms: n(Number(result.latency_ms)) })
            : t('cryptoTrack.network.testOk', {
                name,
                ms: n(Number(result.latency_ms)),
                block: n(Number(result.block)),
              })
        }
        return {
          text: t('cryptoTrack.network.testFailed', { name }),
          description: t(`cryptoTrack.network.outcome.${result.outcome}`),
        }
      },
      error: 'cryptoTrack.network.testRefused',
    },
  })
}

export function useSetConsent() {
  const queryClient = useQueryClient()
  const send = (consent: boolean) => call('crypto_set_tracking_consent', { consent })
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ['crypto'] })
  const grant = useMutation({
    mutationFn: () => send(true),
    onSuccess: refresh,
    meta: {
      success: 'cryptoTrack.consent.granted',
      error: 'cryptoTrack.consent.failed',
      undo: async () => {
        await send(false)
        refresh()
      },
    },
  })
  const withdraw = useMutation({
    mutationFn: () => send(false),
    onSuccess: refresh,
    meta: {
      success: 'cryptoTrack.consent.withdrawn',
      error: 'cryptoTrack.consent.failed',
      undo: async () => {
        await send(true)
        refresh()
      },
    },
  })
  return { grant, withdraw }
}

/** A refusal's reason for a form: the backend's own sentence, never an echo of what was typed. */
export function refusalText(err: unknown): string {
  return parseError(err).message
}
