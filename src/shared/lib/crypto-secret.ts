/**
 * Whether a pasted string looks like a seed phrase or a private key rather than a public
 * address. It lives in `shared` so the form, the feature and the browser mock use the one copy.
 * The Rust side (`looks_like_secret`, `service/cryptotrack/accounts.rs`) holds the same
 * rules and is the one that decides; this copy lets the form refuse a paste before it is ever
 * kept in a field, a query or a request.
 *
 * It errs on the side of refusing: a real address is never several words, never 64 hex digits and
 * never a WIF or an extended private key, so a false alarm costs one paste and a miss costs funds.
 */

const BASE58 = /^[1-9A-HJ-NP-Za-km-z]+$/
const EXTENDED_PREFIXES = ['xprv', 'yprv', 'zprv', 'tprv', 'uprv', 'vprv']

export function looksLikeSecret(text: string): boolean {
  const value = text.trim()
  if (!value) return false
  const words = value.split(/\s+/)
  if (words.length >= 6 && words.every((word) => /^[A-Za-z]{2,12}$/.test(word))) return true
  const hex = value.replace(/^0x/i, '')
  if (hex.length >= 64 && /^[0-9a-fA-F]+$/.test(hex)) return true
  if (EXTENDED_PREFIXES.some((prefix) => value.startsWith(prefix))) return true
  const base58 = BASE58.test(value)
  if (base58 && ((value.length === 51 && value.startsWith('5')) || (value.length === 52 && /^[KL]/.test(value))))
    return true
  if (base58 && (value.length === 87 || value.length === 88)) return true
  return value.startsWith('[') && (value.match(/,/g)?.length ?? 0) >= 31
}

/** The refusal the backend answers with; the form and the mock say the same words. */
export const SECRET_REFUSED =
  'crypto: this looks like a seed phrase or a private key; it was not accepted and not stored'

/** Whether an error from the backend is that refusal. */
export function isSecretRefusal(error: unknown): boolean {
  const message =
    typeof error === 'object' && error !== null && 'message' in error ? String(error.message) : String(error)
  return message.includes('seed phrase or a private key')
}
