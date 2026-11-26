import { type QueryClient, useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

import type { QuarantineRow } from '../crypto-token.types'

const ref = (row: QuarantineRow): TokenRefInput => ({ chain: row.chain, contract: row.contract })

/**
 * A verdict decides whether a token counts at all: the whole `crypto` key tree (accounts, tokens, positions)
 * and every figure read off the holdings (wallet totals, planning) are refetched.
 */
function refreshAfterVerdict(queryClient: QueryClient) {
  for (const queryKey of [['crypto'], ...LEDGER_KEYS, ...PLANNING_KEYS]) {
    void queryClient.invalidateQueries({ queryKey })
  }
}

/** The user's verdict on a quarantined token: recognise or hide. `Undo` returns it to the quarantine. */
export function useTokenReview() {
  const queryClient = useQueryClient()
  const refresh = () => refreshAfterVerdict(queryClient)
  const takeBack = (_data: unknown, row: unknown) =>
    call<TokenStatus>('crypto_clear_token_verdict', { input: ref(row as QuarantineRow) })
  const confirm = useMutation({
    mutationFn: (row: QuarantineRow) => call<TokenStatus>('crypto_confirm_token', { input: ref(row) }),
    onSuccess: refresh,
    meta: { success: 'cryptoTrack.quarantine.recognized', error: 'cryptoTrack.quarantine.failed', undo: takeBack },
  })
  const reject = useMutation({
    mutationFn: (row: QuarantineRow) => call<TokenStatus>('crypto_reject_token', { input: ref(row) }),
    onSuccess: refresh,
    meta: { success: 'cryptoTrack.quarantine.hidden', error: 'cryptoTrack.quarantine.failed', undo: takeBack },
  })
  return { confirm, reject }
}
