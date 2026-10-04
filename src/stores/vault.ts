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
    },
    async loadRecents() {
      this.recents = await call<RecentVault[]>('recent_vaults')
    },
    async selectVault(path: string) {
      const status = await call<VaultStatus>('select_vault', { path })
      this.status = status
      this.locked = status.unlocked !== true
    },
    async forgetRecent(path: string) {
      await call('forget_recent_vault', { path })
      this.recents = this.recents.filter((recent: RecentVault) => recent.path !== path)
    },
    async pickVaultFile(save: boolean) {
      return await call<{ path: string } | null>('pick_vault_file', { save })
    },
    async unlock(password: string) {
      this.busy = true
      this.error = null
      try {
        const status = await call<VaultStatus>('vault_status')
        if (!status) throw new Error('no vault')
        this.info = await call<VaultInfo>('unlock_vault', {
          input: { path: status.path, password },
        })
        this.locked = false
        localStorage.setItem('trade-app:unlocked', 'true')
        this.status = { ...status, unlocked: true, info: this.info }
      } catch (err) {
        this.error = (err as Error).message
        throw err
      } finally {
        this.busy = false
      }
    },
    async createVault(password: string) {
      this.busy = true
      this.error = null
      try {
        const status = await call<VaultStatus>('vault_status')
        this.info = await call<VaultInfo>('create_vault', {
          input: { path: status.path, password },
        })
        this.locked = false
        localStorage.setItem('trade-app:unlocked', 'true')
        this.status = { ...status, exists: true, unlocked: true, info: this.info }
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
