import { describe, expect, it } from 'vitest'

import { vaultActionIds } from './vault-actions'

const recent = (inAppFolder: boolean) => ({ in_app_folder: inAppFolder }) as RecentVault

describe('vaultActionIds', () => {
  it('offers lock, switch, copy and forget', () => {
    expect(vaultActionIds(true, recent(true))).toEqual(['lock', 'switch', 'saveCopy', 'forget'])
  })

  it('has nothing to lock or switch with on a vault without a password', () => {
    expect(vaultActionIds(false, recent(true))).toEqual(['saveCopy', 'forget'])
  })

  it('never offers delete for the open vault, wherever its file lives', () => {
    expect(vaultActionIds(true, recent(false))).toEqual(['lock', 'switch', 'saveCopy', 'forget'])
    expect(vaultActionIds(true, recent(true))).not.toContain('delete')
  })

  it('keeps lock and switch when the history does not know the vault yet', () => {
    expect(vaultActionIds(true, undefined)).toEqual(['lock', 'switch'])
  })
})
