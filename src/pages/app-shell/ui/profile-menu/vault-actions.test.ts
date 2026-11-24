import { describe, expect, it } from 'vitest'

import { vaultActionIds } from './vault-actions'

const recent = (inAppFolder: boolean) => ({ in_app_folder: inAppFolder }) as RecentVault

describe('vaultActionIds', () => {
  it('offers everything for a locked-able vault kept in the app folder', () => {
    expect(vaultActionIds(true, recent(true))).toEqual(['lock', 'switch', 'saveCopy', 'forget', 'delete'])
  })

  it('has nothing to lock or switch with on a vault without a password', () => {
    expect(vaultActionIds(false, recent(true))).toEqual(['saveCopy', 'forget', 'delete'])
  })

  it('offers deleting only a file in the app folder', () => {
    expect(vaultActionIds(true, recent(false))).toEqual(['lock', 'switch', 'saveCopy', 'forget'])
  })

  it('keeps lock and switch when the history does not know the vault yet', () => {
    expect(vaultActionIds(true, undefined)).toEqual(['lock', 'switch'])
  })
})
