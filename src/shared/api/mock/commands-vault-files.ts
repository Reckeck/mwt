/** Commands: picking vault files, recents and the external copy (`platform_fs` in Rust). */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockStatus } from './vault-data'

/** A path with `conflict` in it answers `SYNC_CONFLICT` until it is resolved. */
export const mockConflict = { resolved: false }

/** Linked copy of the mock vault; `null` while none is linked. */
const mockLink: { value: VaultLinkStatus | null } = { value: null }

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
    const path = '/tmp/mock/vault.wallet'
    return resolve<RecentVault[]>([
      {
        path,
        last_opened: String(Math.floor(Date.now() / 1000)),
        exists: true,
        display_name: nameOf(path),
        linked: mockLink.value !== null,
      },
    ])
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
      last_error: null,
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
