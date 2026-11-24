export type VaultActionId = 'lock' | 'switch' | 'saveCopy' | 'forget' | 'delete'

/**
 * The vault actions the profile menu lists, in order. Lock and switch need a password to come
 * back with; the rest work on the history row of the open vault, and only a file in the app's
 * own folder can be erased.
 */
export function vaultActionIds(encrypted: boolean, recent: RecentVault | undefined): VaultActionId[] {
  const ids: VaultActionId[] = encrypted ? ['lock', 'switch'] : []
  if (!recent) return ids
  ids.push('saveCopy', 'forget')
  if (recent.in_app_folder) ids.push('delete')
  return ids
}
