/**
 * The rules for a new vault secret, the same as in the core
 * (`crates/core/src/service/security_level.rs`): a level-2 PIN is six digits or more,
 * a level-3 password is 16 characters or more and not weak. An older shorter PIN still opens its vault.
 */

/** Shortest new level-2 PIN (digits only). */
export const MIN_PIN_DIGITS = 6
/** Shortest master password a form accepts for a new level-2 vault: the PIN length. */
export const MIN_VAULT_PASSWORD = MIN_PIN_DIGITS
/** Shortest level-3 password. */
export const MIN_CRYPTO_PASSWORD = 16

/** Whether `pin` may be set as a new level-2 PIN. */
export function pinOk(pin: string): boolean {
  return pin.length >= MIN_PIN_DIGITS && /^[0-9]+$/.test(pin)
}

/** The strength of a level-3 password and whether it is long enough, as `validate_password_strength` answers. */
export function passwordStrength(password: string): PasswordStrengthReport {
  const chars = Array.from(password)
  return { strength: grade(chars), long_enough: chars.length >= MIN_CRYPTO_PASSWORD }
}

/** Fill of a strength meter, percent. */
export const STRENGTH_PERCENT: Record<PasswordStrength, number> = { weak: 25, medium: 50, strong: 100 }

/** Whether `password` may protect a level-3 vault. */
export function cryptoPasswordOk(password: string): boolean {
  const report = passwordStrength(password)
  return report.long_enough && report.strength !== 'weak'
}

const isLower = (char: string) => /\p{Lowercase}/u.test(char)
const isUpper = (char: string) => /\p{Uppercase}/u.test(char)
const isDigit = (char: string) => char >= '0' && char <= '9'

/** Short, few distinct characters or mostly runs is weak; long with several kinds is strong. */
function grade(chars: string[]): PasswordStrength {
  const count = chars.length
  if (count < MIN_CRYPTO_PASSWORD) return 'weak'
  const distinct = new Set(chars).size
  let runs = 0
  for (let index = 1; index < count; index += 1) {
    const step = (chars[index].codePointAt(0) ?? 0) - (chars[index - 1].codePointAt(0) ?? 0)
    if (Math.abs(step) <= 1) runs += 1
  }
  if (distinct < 8 || runs * 2 >= count - 1) return 'weak'
  const classes = [
    chars.some(isLower),
    chars.some(isUpper),
    chars.some(isDigit),
    chars.some((char) => !isLower(char) && !isUpper(char) && !isDigit(char)),
  ].filter(Boolean).length
  if ((count >= 20 && classes >= 3 && distinct >= 12) || (count >= 24 && distinct >= 10)) return 'strong'
  return 'medium'
}
