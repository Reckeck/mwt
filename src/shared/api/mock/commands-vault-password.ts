/** Commands: password of the open vault (`set_vault_password`, `remove_vault_password` in Rust), with state. */

import { dropPlainBackups } from './commands-vault-backups'
import { mockLink } from './commands-vault-files'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockInfo, mockPassword, mockStatus } from './vault-data'

/** The vault and the status screen both say whether there is a password. */
function setEncrypted(encrypted: boolean) {
  mockInfo.encrypted = encrypted
  mockStatus.value = { ...mockStatus.value, encrypted }
}

const invalid = (message: string) => ({ code: 'INVALID_INPUT', message })

/** Commands: vault password. Same checks and order as the core: check first, change after. */
export const vaultPasswordCommands: MockCommands = {
  set_vault_password: ({ body }) => {
    if (mockPassword.value !== null) throw invalid('the vault already has a password: change it')
    if (!body?.new_password) throw invalid('the new password must not be empty')
    mockPassword.value = String(body.new_password)
    setEncrypted(true)
    return resolve<SetVaultPasswordResult>({ removed_copies: body.delete_plain_copies ? dropPlainBackups() : 0 })
  },
  remove_vault_password: ({ body }) => {
    if (mockPassword.value === null) throw invalid('the vault has no password')
    if (body?.current_password !== mockPassword.value) throw { code: 'SECRET', message: 'Wrong password' }
    // A vault with an external copy keeps its password unless the caller confirms the copy loses it too.
    if (mockLink.value !== null && !body?.external_copy_too) {
      throw invalid('the vault has an external copy that would lose its password too: confirm it')
    }
    mockPassword.value = null
    setEncrypted(false)
    return resolve(undefined)
  },
}
