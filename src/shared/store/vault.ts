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
    /** The open vault has a password; `false` hides `Lock` and the auto-lock (nothing to lock with). */
    encrypted: (state) => state.status?.encrypted !== false,
    /** The open vault is the sandbox (demo data, erased on exit): the shell shows its bar, the network features wait. */
    sandbox: (state) => state.status?.sandbox === true,
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
    /** Returns the index the row had (for `restoreRecent`), `-1` when it was not listed. */
    async forgetRecent(path: string) {
      await call('forget_recent_vault', { input: { path } })
      const index = this.recents.findIndex((recent: RecentVault) => recent.path === path)
      this.recents = this.recents.filter((recent: RecentVault) => recent.path !== path)
      return index
    },
    /** Undo of `forgetRecent`: the row returns to `index`. */
    async restoreRecent(path: string, index: number) {
      await call('restore_recent_vault', { input: { path, index: Math.max(index, 0) } })
      await this.loadRecents()
    },
    /** `Find file…`: the row of `path` points at `newPath` and keeps its place in the list. */
    async relocateRecent(path: string, newPath: string) {
      await call('relocate_recent_vault', { input: { path, new_path: newPath } })
      await this.loadRecents()
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
      return await this.openWith('unlock_vault', { path, password })
    },
    /** Quick unlock (H7A): the OS releases the password after Touch ID or Windows Hello, Rust opens the vault with it. */
    async unlockByBiometric(path: string) {
      return await this.openWith('biometric_unlock', { path })
    },
    async openWith(command: 'unlock_vault' | 'biometric_unlock', input: { path: string; password?: string }) {
      this.busy = true
      this.error = null
      try {
        this.info = await call<VaultInfo>(command, { input })
        this.locked = false
        localStorage.setItem('trade-app:unlocked', 'true')
        this.status = {
          path: this.info.path,
          exists: true,
          unlocked: true,
          info: this.info,
          storage: this.status?.storage ?? 'files',
          encrypted: this.info.encrypted,
          sandbox: false,
        }
      } catch (err) {
        this.error = (err as Error).message
        throw err
      } finally {
        this.busy = false
      }
    },
    /** `crypto`: the vault keeps crypto wallets and exchanges (asked on create); `password: null` — a vault without a password. */
    async createVault(path: string, password: string | null, crypto: boolean) {
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
          encrypted: this.info.encrypted,
          sandbox: false,
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
    /** `Open the sandbox`: the demo vault opens in place of the unlock screen. Not remembered as the last vault. */
    async openSandbox() {
      return await this.enterSandbox('open_sandbox')
    },
    /** `Reset`: the same demo data again, whatever was done in the sandbox. */
    async resetSandbox() {
      return await this.enterSandbox('reset_sandbox')
    },
    async enterSandbox(command: 'open_sandbox' | 'reset_sandbox') {
      this.busy = true
      try {
        this.info = await call<VaultInfo>(command)
        await this.refreshStatus()
        return this.info
      } finally {
        this.busy = false
      }
    },
    /** `Exit`: the sandbox is closed and erased; the unlock screen follows (`locked`). */
    async closeSandbox() {
      await call('close_sandbox')
      this.info = null
      await this.refreshStatus()
    },
    /** The backend locked or unlocked the vault by itself (`vault-changed`, subscribed in App.vue). */
    applyStatus(status: VaultStatus) {
      this.status = status
      this.locked = status.unlocked !== true
    },
  },
})
