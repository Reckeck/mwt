import { parseError, type AppError } from '@/shared/api/ipc'

/** Minimal shape of vue-i18n's `t` — keeps this helper framework-agnostic. */
type Translate = (key: string) => string

/** Vault error codes that deserve their own wording instead of the raw message. */
const VAULT_ERROR_KEYS: Record<string, string> = {
  SECRET: 'auth.wrongPassword',
  ALREADY_EXISTS: 'auth.pathTaken',
  UNSUPPORTED: 'auth.unsupported',
  NOT_FOUND: 'auth.missing',
  INCOMPATIBLE_VAULT: 'vault.incompatible',
  VAULT_BUSY: 'auth.vaultBusy',
}

/**
 * Reads an `AppError` into a translated message.
 *
 * Unknown codes fall through to the backend message: the UI must never show a
 * raw `CODE: message` string when the vault is the thing that failed.
 */
export function vaultErrorMessage(err: unknown, t: Translate): string {
  const { code, message } = parseError(err)
  const key = VAULT_ERROR_KEYS[code]
  if (!key) return message
  const known = t(key)
  return known === key ? message : known
}

/** Same mapping for an already-parsed error. */
export function appErrorMessage(err: AppError, t: Translate): string {
  const key = VAULT_ERROR_KEYS[err.code]
  if (!key) return err.message
  const known = t(key)
  return known === key ? err.message : known
}

/**
 * Localize any backend error by its code through the `errors.*` namespace,
 * falling back to the raw message when the code has no translation.
 */
/** Codes whose backend text carries the specifics (amounts, names) worth keeping. */
const DETAILED_CODES = new Set(['INSUFFICIENT_FUNDS'])

export function codeErrorMessage(err: AppError, t: Translate): string {
  const key = `errors.${err.code}`
  const known = t(key)
  if (known === key) return err.message
  return DETAILED_CODES.has(err.code) ? `${known}: ${err.message}` : known
}
