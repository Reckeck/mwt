/** Commands: the move to the crypto level. The mock checks the PIN and the password rule as the backend does. */

import { passwordStrength, cryptoPasswordOk } from '@/shared/lib/vault-password'

import { resolve, type MockCommands } from './core'
import { mockPassword } from './vault-data'
import { mockSecurityLevel } from './commands-security-state'

const reject = (code: string, message: string) => Promise.reject({ code, message })

export const securityRaiseCommands: MockCommands = {
  validate_password_strength: ({ body }) => resolve(passwordStrength((body as { password: string }).password ?? '')),
  raise_to_crypto_level: ({ body }) => {
    const input = body as RaiseToCryptoInput
    if (mockSecurityLevel.value === 'crypto') return reject('INVALID_INPUT', 'the vault is already at level 3')
    if (input.backup_path) return reject('INVALID_INPUT', 'the backup is kept next to the vault')
    // A vault without a password goes straight to level 3: there is nothing to check.
    if (mockPassword.value !== null && input.current_password !== mockPassword.value)
      return reject('SECRET', 'wrong password or tampered file')
    if (!cryptoPasswordOk(input.new_password))
      return reject('PASSWORD_TOO_WEAK', 'the password is too short or too weak for level 3')
    mockPassword.value = input.new_password
    mockSecurityLevel.value = 'crypto'
    const report: RaiseToCryptoReport = { level: 'crypto', backup_path: '' }
    return resolve(report)
  },
}
