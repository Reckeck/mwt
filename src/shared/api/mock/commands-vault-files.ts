/** Commands: picking vault files, recents and the external copy (`platform_fs` in Rust). */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockInfo, mockStatus } from './vault-data'

/** A path with `conflict` in it answers `SYNC_CONFLICT` until it is resolved. */
export const mockConflict = { resolved: false }

/** Linked copy of the mock vault; `null` while none is linked. */
export const mockLink: { value: VaultLinkStatus | null } = { value: null }

/** A history row of the mock; the shots and `pnpm dev` bend these (missing file, no password). */
export interface MockRecent {
  path: string
  /** Unix seconds. */
  opened: number
  exists: boolean
  encrypted: boolean
}

const DAY = 86400

/** Recent vaults of the mock, newest first (Full-New-Recents: «vault», «семья», «old-2024»). */
export const mockRecents: MockRecent[] = [
  { path: '/home/anna/Документы/vault.wallet', opened: Math.floor(Date.now() / 1000), exists: true, encrypted: true },
  {
    path: '/home/anna/Документы/Семья/семья.wallet',
    opened: Math.floor(Date.now() / 1000) - 13 * DAY,
    exists: true,
    encrypted: true,
  },
  { path: '/home/anna/Загрузки/old-2024.wallet', opened: 1_720_000_000, exists: false, encrypted: true },
]

/** Rows taken out of the history, for "Undo". */
const forgotten = new Map<string, MockRecent>()

const reject = (code: string, message: string) => Promise.reject({ code, message })

const indexOfPath = (path: unknown) => mockRecents.findIndex((row) => row.path === path)

const nameOf = (path: string) => path.split(/[\\/]/).pop() || path

/** Commands: vault files. */
export const vaultFilesCommands: MockCommands = {
  pick_vault_file: ({ args }) => {
    // No native dialog in the browser mock; report a plausible pick so the
    // create/open flows stay observable during `pnpm dev`.
    const app = mockStatus.value.storage === 'app'
    const path = args?.save ? '/tmp/mock/new-vault.wallet' : '/tmp/mock/vault.wallet'
    return resolve<PickedVault>({ path, display_name: nameOf(path), imported: app })
  },
  recent_vaults: (_call) => {
    const app = mockStatus.value.storage === 'app'
    return resolve<RecentVault[]>(
      mockRecents.map((row, index) => ({
        path: row.path,
        last_opened: String(row.opened),
        exists: row.exists,
        display_name: nameOf(row.path),
        linked: mockLink.value !== null && index === 0,
        encrypted: index === 0 ? mockInfo.encrypted && row.encrypted : row.encrypted,
        in_app_folder: app,
      })),
    )
  },
  forget_recent_vault: ({ body }) => {
    const index = indexOfPath(body?.path)
    if (index >= 0) {
      const [row] = mockRecents.splice(index, 1)
      forgotten.set(row.path, row)
    }
    return resolve(undefined)
  },
  // Undo: the row returns to its old place (what was dropped is kept for it).
  restore_recent_vault: ({ body }) => {
    const path = String(body?.path)
    if (indexOfPath(path) < 0) {
      const row = forgotten.get(path) ?? { path, opened: Math.floor(Date.now() / 1000), exists: true, encrypted: true }
      mockRecents.splice(Math.min(Number(body?.index ?? 0), mockRecents.length), 0, row)
    }
    return resolve(undefined)
  },
  // "Find file…": the row points at the found file and keeps its place.
  relocate_recent_vault: ({ body }) => {
    const index = indexOfPath(body?.path)
    const target = String(body?.new_path)
    if (index < 0 || target === body?.path) return resolve(undefined)
    const other = indexOfPath(target)
    if (other >= 0) mockRecents.splice(other, 1)
    const row = mockRecents[indexOfPath(body?.path)]
    row.path = target
    row.exists = true
    return resolve(undefined)
  },
  delete_vault_file: ({ body }) => {
    const path = String(body?.path)
    if (mockStatus.value.storage !== 'app') return reject('INVALID_INPUT', 'the file is not in the app folder')
    if (indexOfPath(path) < 0) return reject('NOT_FOUND', path)
    if (mockStatus.value.unlocked && mockStatus.value.path === path) {
      return reject('INVALID_INPUT', 'the open vault cannot be deleted')
    }
    mockRecents.splice(indexOfPath(path), 1)
    return resolve<DeleteVaultFileOutput>({ file_name: nameOf(path), backups: [`${nameOf(path)}.bak`] })
  },
  export_vault_copy: ({ body }) => {
    const path = String(body?.path)
    return resolve<string | null>(`Download/${nameOf(path)}`)
  },
  vault_link_status: (_call) => {
    return resolve<VaultLinkStatus>(
      mockLink.value ?? { linked: false, display_name: null, last_synced_at: null, last_error: null },
    )
  },
  link_vault_external: ({ body }) => {
    mockLink.value = {
      linked: true,
      display_name: body?.existing ? 'vault-copy.wallet' : 'Download/vault.wallet',
      last_synced_at: new Date().toISOString(),
      // Demo: «Select existing» shows the state of a copy changed on another device.
      last_error: body?.existing ? 'VAULT_REMOTE_CHANGED: the external copy was changed on another device' : null,
    }
    return resolve(mockLink.value)
  },
  unlink_vault_external: (_call) => {
    mockLink.value = null
    return resolve({})
  },
  resolve_vault_conflict: (_call) => {
    mockConflict.resolved = true
    return resolve({})
  },
}
