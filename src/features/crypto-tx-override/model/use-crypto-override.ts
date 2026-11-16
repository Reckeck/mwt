import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { CRYPTO_OPERATIONS_KEY } from '@/entities/crypto-tx'
import { call } from '@/shared/api/ipc'

/**
 * Every write of this feature is `crypto_set_override`: a correction of classification or attributes over an
 * operation the module read from its source. There is no mutation here that adds or removes an operation, and
 * the backend has no command that would.
 */
export type OverrideRequest = Pick<CryptoSetOverrideInput, 'event_id'> & Partial<CryptoSetOverrideInput>
const setOverride = (input: OverrideRequest) =>
  call<CryptoSetOverrideResult>('crypto_set_override', { input: { clear: [], same_contract: false, ...input } })

/** A correction changes holdings (a chain decides a cost basis), flows and the contact's saved addresses. */
function useAfterOverride() {
  const queryClient = useQueryClient()
  return () => {
    for (const queryKey of [
      CRYPTO_OPERATIONS_KEY,
      ['transactions'],
      ['flows'],
      ['summary'],
      ['contacts'],
      ['report'],
    ]) {
      void queryClient.invalidateQueries({ queryKey: [...queryKey] })
    }
  }
}

/** The category, the description and the counterparty of one operation, or of every one of its contract. */
export function useSaveMeaning(onDone?: () => void) {
  const { t } = useI18n()
  const after = useAfterOverride()
  return useMutation({
    mutationFn: (input: OverrideRequest) => setOverride(input),
    onSuccess: () => {
      after()
      onDone?.()
    },
    meta: {
      error: 'cryptoTrack.operations.saveFailed',
      success: (data: unknown, input: unknown) => {
        const done = data as CryptoSetOverrideResult
        const wide = (input as OverrideRequest).same_contract && done.applied > 1
        return t(wide ? 'cryptoTrack.operations.savedMany' : 'cryptoTrack.operations.saved', { count: done.applied })
      },
    },
  })
}

/** Hide an operation from the lists and the totals, or show it again; reversible from the notification. */
export function useHideOperation() {
  const after = useAfterOverride()
  return useMutation({
    mutationFn: (variables: { id: string; hidden: boolean }) =>
      setOverride({ event_id: variables.id, hidden: variables.hidden }),
    onSuccess: after,
    meta: {
      error: 'cryptoTrack.operations.saveFailed',
      success: (_data: unknown, variables: unknown) =>
        (variables as { hidden: boolean }).hidden
          ? 'cryptoTrack.operations.hiddenToast'
          : 'cryptoTrack.operations.shownToast',
      undo: (_data: unknown, variables: unknown) => {
        const { id, hidden } = variables as { id: string; hidden: boolean }
        return setOverride({ event_id: id, hidden: !hidden })
      },
    },
  })
}

/**
 * «This is not an exchange»: the app stops suggesting a fiat operation for this one. The answer is stored
 * with the data, not in this browser, so it survives a reinstall and reaches the other devices; the
 * notification takes it back, and then the guess may be offered again.
 */
export function useDismissSuggestion() {
  const after = useAfterOverride()
  return useMutation({
    mutationFn: (variables: { id: string }) => setOverride({ event_id: variables.id, not_exchange: true }),
    onSuccess: after,
    meta: {
      error: 'cryptoTrack.operations.saveFailed',
      success: 'cryptoTrack.operations.notExchangeToast',
      undo: (_data: unknown, variables: unknown) =>
        setOverride({ event_id: (variables as { id: string }).id, not_exchange: false }),
    },
  })
}

/**
 * Take the rule off this operation's contract. The operations it already classified keep what they have
 * only where the user corrected them one by one: the rest go back to what their source said.
 */
export function useRemoveContractRule() {
  const after = useAfterOverride()
  return useMutation({
    mutationFn: (variables: { id: string }) => setOverride({ event_id: variables.id, remove_contract_rule: true }),
    onSuccess: after,
    meta: { error: 'cryptoTrack.operations.saveFailed', success: 'cryptoTrack.operations.ruleRemoved' },
  })
}

/** «This is an exchange»: one chain with the fiat operation the user picked. Nothing is linked by itself. */
export function useLinkExchange(onDone?: () => void) {
  const after = useAfterOverride()
  return useMutation({
    mutationFn: (variables: { id: string; transactionId: string }) =>
      setOverride({ event_id: variables.id, link_transaction_id: variables.transactionId }),
    onSuccess: () => {
      after()
      onDone?.()
    },
    meta: {
      error: 'cryptoTrack.operations.linkFailed',
      success: 'cryptoTrack.operations.linked',
      undo: (_data: unknown, variables: unknown) =>
        setOverride({ event_id: (variables as { id: string }).id, clear: ['flow'] }),
    },
  })
}

/** Break the link; the notification puts it back. */
export function useUnlinkExchange() {
  const after = useAfterOverride()
  return useMutation({
    mutationFn: (variables: { id: string; transactionId?: string }) =>
      setOverride({ event_id: variables.id, clear: ['flow'] }),
    onSuccess: after,
    meta: {
      error: 'cryptoTrack.operations.linkFailed',
      success: 'cryptoTrack.operations.unlinked',
      undo: (_data: unknown, variables: unknown) => {
        const { id, transactionId } = variables as { id: string; transactionId?: string }
        return transactionId ? setOverride({ event_id: id, link_transaction_id: transactionId }) : Promise.resolve(null)
      },
    },
  })
}
