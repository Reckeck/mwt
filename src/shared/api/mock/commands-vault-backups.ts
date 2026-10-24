/** Commands: backup copies of the vault file (`vault_backups` in Rust), with a list that changes. */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockInfo } from './vault-data'

/** Newest first, like the real list. The seed is the canvas's «Последняя: vault.wallet.v2.bak · 21 окт». */
const mockBackups: VaultBackup[] = [
  { name: 'vault.wallet.v2.bak', modified_at: '2026-10-21T09:41:00Z', size_bytes: 48_213 },
  { name: 'vault.wallet.bak', modified_at: '2026-10-12T18:05:00Z', size_bytes: 41_877 },
]

/** Copies taken while the vault had no password: the ones «delete old copies without a password» removes. */
export const plainBackups = new Set<string>()
export function dropPlainBackups(): number {
  const gone = mockBackups.filter((item) => plainBackups.has(item.name))
  for (const item of gone) mockBackups.splice(mockBackups.indexOf(item), 1)
  plainBackups.clear()
  return gone.length
}

const stamp = () =>
  new Date()
    .toISOString()
    .replace(/\D/g, '')
    .replace(/^(\d{8})(\d{6}).*/, '$1-$2')

/** A backup copy of "the file as it is now", named so that it never replaces an earlier one. */
function addBackup(): VaultBackup {
  const base = `vault.wallet.${stamp()}`
  let name = `${base}.bak`
  for (let attempt = 2; mockBackups.some((item) => item.name === name); attempt++) name = `${base}-${attempt}.bak`
  const backup = { name, modified_at: new Date().toISOString(), size_bytes: 49_000 }
  mockBackups.unshift(backup)
  if (!mockInfo.encrypted) plainBackups.add(name)
  return backup
}

/** Commands: vault backups. */
export const vaultBackupsCommands: MockCommands = {
  list_vault_backups: (_call) => resolve<VaultBackup[]>(mockBackups),
  create_vault_backup: (_call) => resolve<VaultBackup>(addBackup()),
  restore_vault_backup: ({ body }) => {
    // The same rule as in Rust: only a name from the list, nothing else.
    if (!mockBackups.some((item) => item.name === body?.name)) {
      return Promise.reject(`INVALID_INPUT: '${String(body?.name)}' is not a backup of this vault`)
    }
    return resolve<VaultBackup>(addBackup())
  },
  reveal_vault_folder: (_call) => resolve(undefined),
}
