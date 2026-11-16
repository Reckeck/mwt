import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { CRYPTO_OPERATIONS_KEY } from '@/entities/crypto-tx'
import { call } from '@/shared/api/ipc'

/**
 * Pointing a fiat record at a crypto operation (spec E13). The pointer is a visible reference only: it joins
 * no chain and changes no sum, so nothing but the crypto queries is refreshed. The chain of money is
 * `crypto_set_override`'s, in the crypto-tx-override feature.
 */
export interface LinkChange {
  event_id: string
  fiat_tx_id: string
  /** What the operation pointed at before, so an undo puts it back. */
  previous?: string | null
}

const link = (change: LinkChange) =>
  call<void>('crypto_link_operation', { input: { event_id: change.event_id, fiat_tx_id: change.fiat_tx_id } })
const unlink = (eventId: string) => call<void>('crypto_unlink_operation', { input: { event_id: eventId } })

function useAfterLink() {
  const queryClient = useQueryClient()
  return () => void queryClient.invalidateQueries({ queryKey: [...CRYPTO_OPERATIONS_KEY] })
}

/** Save the pointer. The record must be saved first: a new one has no id before that. */
export function useLinkCryptoOperation() {
  return useMutation({
    mutationFn: link,
    onSuccess: useAfterLink(),
    meta: {
      success: 'cryptoTrack.links.linked',
      error: 'cryptoTrack.links.linkFailed',
      undo: (_data: unknown, variables: unknown) => {
        const change = variables as LinkChange
        return change.previous ? link({ ...change, fiat_tx_id: change.previous }) : unlink(change.event_id)
      },
    },
  })
}

/** Remove the pointer; the notification's undo puts it back. */
export function useUnlinkCryptoOperation() {
  return useMutation({
    mutationFn: (change: LinkChange) => unlink(change.event_id),
    onSuccess: useAfterLink(),
    meta: {
      success: 'cryptoTrack.links.unlinked',
      error: 'cryptoTrack.links.unlinkFailed',
      undo: (_data: unknown, variables: unknown) => link(variables as LinkChange),
    },
  })
}

/** Crypto operations that look like the same movement of money as the form's record: only a suggestion. */
export function useCryptoLinkCandidates(
  input: MaybeRefOrGetter<CryptoLinkCandidatesInput>,
  enabled: MaybeRefOrGetter<boolean>,
) {
  return useQuery({
    queryKey: computed(() => [...CRYPTO_OPERATIONS_KEY, 'link-candidates', toValue(input)]),
    queryFn: () => call<CryptoLinkOption[]>('crypto_link_candidates', { input: toValue(input) }),
    enabled: computed(() => toValue(enabled)),
    placeholderData: keepPreviousData,
  })
}
