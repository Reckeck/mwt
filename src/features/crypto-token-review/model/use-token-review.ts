import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { CRYPTO_ACCOUNTS_KEY } from '@/entities/crypto-account'
import { CRYPTO_TOKENS_KEY, type QuarantineRow } from '@/entities/crypto-token'
import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

const ref = (row: QuarantineRow): TokenRefInput => ({ chain: row.chain, contract: row.contract })

/**
 * The user's verdict on a quarantined token. A verdict decides whether the token counts at all, so
 * the module's lists and every figure read off the holdings are refetched.
 */
export function useTokenReview() {
  const queryClient = useQueryClient()
  const refresh = () => {
    for (const queryKey of [CRYPTO_ACCOUNTS_KEY, CRYPTO_TOKENS_KEY, ...LEDGER_KEYS, ...PLANNING_KEYS]) {
      void queryClient.invalidateQueries({ queryKey })
    }
  }
  // `Undo` takes the verdict back: the token returns to the quarantine and leaves the capital again.
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
  // The lasting way back, from the assets table: the token leaves the capital and waits again. `Undo`
  // recognises it once more.
  const clear = useMutation({
    mutationFn: (token: TokenRefInput) => call<TokenStatus>('crypto_clear_token_verdict', { input: token }),
    onSuccess: refresh,
    meta: {
      success: 'cryptoTrack.assets.takenBack',
      error: 'cryptoTrack.assets.takeBackFailed',
      undo: (_data: unknown, token: unknown) =>
        call<TokenStatus>('crypto_confirm_token', { input: token as TokenRefInput }),
    },
  })
  return { confirm, reject, clear }
}
