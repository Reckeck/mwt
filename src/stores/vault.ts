import { defineStore } from 'pinia'

import { call, onEvent } from '@/lib/ipc'

interface VaultState {
  status: VaultStatus | null
  info: VaultInfo | null
  locked: boolean
  error: string | null
  busy: boolean
  recents: RecentVault[]
}

const empty = (): VaultState => ({
  status: null,
  info: null,
  locked: true,
  error: null,
  busy: false,
  recents: [],
})

export const useVaultStore = defineStore('vault', {
  state: empty,
  actions: {
    async refreshStatus() {
      this.status = await call<VaultStatus>('vault_status')
      this.locked = this.status?.unlocked !== true
      return this.status
    },
    async loadRecents() {
      this.recents = await call<RecentVault[]>('recent_vaults')
    },
    async forgetRecent(path: string) {
      await call('forget_recent_vault', { path })
      this.recents = this.recents.filter((recent: RecentVault) => recent.path !== path)
    },
    /** Resolves to the chosen path, or `null` when the dialog was cancelled. */
    async pickVaultFile(save: boolean) {
      return await call<string | null>('pick_vault_file', { save })
    },
    /** `path` is explicit: the unlock screen may target any vault, not just the
     *  default one, and `select_vault` cannot be used for a file that does not
     *  exist yet (creation). */
    async unlock(path: string, password: string) {
      this.busy = true
      this.error = null
      try {
        this.info = await call<VaultInfo>('unlock_vault', {
          input: { path, password },
        })
        this.locked = false
        localStorage.setItem('trade-app:unlocked', 'true')
        this.status = { path, exists: true, unlocked: true, info: this.info }
      } catch (err) {
        this.error = (err as Error).message
        throw err
      } finally {
        this.busy = false
      }
    },
    async createVault(path: string, password: string) {
      this.busy = true
      this.error = null
      try {
        this.info = await call<VaultInfo>('create_vault', {
          input: { path, password },
        })
        this.locked = false
        localStorage.setItem('trade-app:unlocked', 'true')
        this.status = { path, exists: true, unlocked: true, info: this.info }
      } catch (err) {
        this.error = (err as Error).message
        throw err
      } finally {
        this.busy = false
      }
    },
    async lock() {
      await call('lock_vault_cmd')
      this.locked = true
      const status = await call<VaultStatus>('vault_status')
      this.status = status
      localStorage.setItem('trade-app:unlocked', 'false')
    },
    listen() {
      return onEvent<VaultStatus>('vault-changed', (status) => {
        this.status = status
        this.locked = status.unlocked !== true
      })
    },
  },
})
