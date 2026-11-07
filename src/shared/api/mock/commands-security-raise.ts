/** Commands: the move to the crypto level. The mock checks the PIN and the password rule as the backend does. */

import { passwordStrength, cryptoPasswordOk } from '@/shared/lib/vault-password'

import { emitMock } from '../mock-bus'
import { resolve, type MockCommands } from './core'
import { mockPassword } from './vault-data'
import { mockKeyLevelRequired } from './commands-security-keys'
import { mockSecurityLevel } from './commands-security-state'

const reject = (code: string, message: string) => Promise.reject({ code, message })

const STAGES: RaiseStage[] = ['backup', 'reencrypt', 'verify']
/** Each stage takes a moment, as the real re-encryption does, so the wizard's progress can be seen. */
const STAGE_MS = 250
const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms))

/** The run with `mockRaiseFailure.stage` set stops there with a refusal, to see the failure step by hand. */
export const mockRaiseFailure: { stage: RaiseStage | null } = { stage: null }

export const securityRaiseCommands: MockCommands = {
  validate_password_strength: ({ body }) => resolve(passwordStrength((body as { password: string }).password ?? '')),
  raise_to_crypto_level: async ({ body }) => {
    const input = body as RaiseToCryptoInput
    if (mockSecurityLevel.value === 'crypto') return reject('INVALID_INPUT', 'the vault is already at level 3')
    if (input.backup_path) return reject('INVALID_INPUT', 'the backup is kept next to the vault')
    // A vault without a password goes straight to level 3: there is nothing to check.
    if (mockPassword.value !== null && input.current_password !== mockPassword.value)
      return reject('SECRET', 'wrong password or tampered file')
    if (!cryptoPasswordOk(input.new_password))
      return reject('PASSWORD_TOO_WEAK', 'the password is too short or too weak for level 3')
    for (const stage of STAGES) {
      emitMock('security-raise-progress', { stage } satisfies RaiseProgress)
      await pause(STAGE_MS)
      if (mockRaiseFailure.stage === stage) return reject('IO', `the ${stage} stage failed`)
    }
    mockPassword.value = input.new_password
    mockSecurityLevel.value = 'crypto'
    // The wallets' keys work from now on.
    mockKeyLevelRequired.value = false
    const report: RaiseToCryptoReport = { level: 'crypto', backup_path: '' }
    return resolve(report)
  },
}
