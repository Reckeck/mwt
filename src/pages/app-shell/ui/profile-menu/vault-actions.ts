export type VaultActionId = 'lock' | 'switch' | 'saveCopy' | 'forget'

/**
 * The vault actions the profile menu lists, in order. Lock and switch need a password to come
 * back with; the rest work on the history row of the open vault. Delete is not offered: the
 * backend refuses to erase the open vault (it is erased from the unlock screen).
 */
export function vaultActionIds(encrypted: boolean, recent: RecentVault | undefined): VaultActionId[] {
  const ids: VaultActionId[] = encrypted ? ['lock', 'switch'] : []
  if (!recent) return ids
  ids.push('saveCopy', 'forget')
  return ids
}
