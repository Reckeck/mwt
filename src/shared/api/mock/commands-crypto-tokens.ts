/**
 * Commands: the tokens the module met, their quarantine and the verdict on them.
 *
 * Owner: spec E2 (Rust), whose browser mock comes with spec E9. It keeps the list in memory and
 * answers like the application: a new contract starts in quarantine, a verdict moves it out, and the
 * list holds every token with its status (the settings list of hidden tokens, spec E17, filters it).
 */

import { type MockCommands, resolve } from './core'

/** The tokens of the browser session, shared with the accounts mock (what each address holds of them). */
export const mockCryptoTokens: TokenView[] = []

const find = (input: TokenRefInput) =>
  mockCryptoTokens.find((token) => token.chain === input.chain && token.contract === input.contract.toLowerCase())

function decide(call: { body: Record<string, unknown> | undefined }, status: TokenStatus) {
  const token = find(call.body as unknown as TokenRefInput)
  if (!token) return Promise.reject({ code: 'NOT_FOUND', message: 'token' })
  token.status = status
  // Like the backend: the date goes with the verdict, and is gone when the verdict is taken back.
  token.decided_at = status === 'confirmed' || status === 'rejected' ? new Date().toISOString() : undefined
  return resolve(status)
}

export const cryptoTokensCommands: MockCommands = {
  crypto_list_tokens: () => resolve(mockCryptoTokens),
  crypto_confirm_token: (call) => decide(call, 'confirmed'),
  crypto_reject_token: (call) => decide(call, 'rejected'),
  // Taking a verdict back: the demo tokens are all unknown contracts, so they return to the quarantine.
  crypto_clear_token_verdict: (call) => decide(call, 'quarantined'),
  crypto_set_token_basis: ({ body }) => {
    const input = body as unknown as SetTokenBasisInput
    const token = find(input)
    if (!token) return Promise.reject({ code: 'NOT_FOUND', message: 'token' })
    if (input.basis_usd !== null && Number(input.basis_usd) < 0) {
      return Promise.reject({ code: 'INVALID_INPUT', message: 'crypto: a cost basis cannot be negative' })
    }
    token.basis_usd = input.basis_usd
    return resolve(null)
  },
}
