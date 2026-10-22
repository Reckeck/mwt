import { defineStore } from 'pinia'

import { call } from '@/shared/api/ipc'

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
  getters: {
    /** Android/iOS: vaults live in app storage, a picked file is copied in. */
    appStorage: (state) => state.status?.storage === 'app',
  },
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
    /** The chosen vault (on mobile already copied into app storage), or
     *  `null` when the dialog was cancelled. */
    async pickVaultFile(save: boolean) {
      return await call<PickedVault | null>('pick_vault_file', { save })
    },
    /** After `SYNC_CONFLICT`: make the vault and its external copy equal. */
    async resolveConflict(path: string, keep: ConflictSide) {
      await call('resolve_vault_conflict', { input: { path, keep } })
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
        this.status = {
          path: this.info.path,
          exists: true,
          unlocked: true,
          info: this.info,
          storage: this.status?.storage ?? 'files',
        }
      } catch (err) {
        this.error = (err as Error).message
        throw err
      } finally {
        this.busy = false
      }
    },
    /** `crypto`: the vault keeps crypto wallets and exchanges (asked on create). */
    async createVault(path: string, password: string, crypto: boolean) {
      this.busy = true
      this.error = null
      try {
        this.info = await call<VaultInfo>('create_vault', {
          input: { path, password, cash_name: null, crypto },
        })
        this.locked = false
        localStorage.setItem('trade-app:unlocked', 'true')
        this.status = {
          path: this.info.path,
          exists: true,
          unlocked: true,
          info: this.info,
          storage: this.status?.storage ?? 'files',
        }
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
    /** The backend locked or unlocked the vault by itself (`vault-changed`, subscribed in App.vue). */
    applyStatus(status: VaultStatus) {
      this.status = status
      this.locked = status.unlocked !== true
    },
  },
})
