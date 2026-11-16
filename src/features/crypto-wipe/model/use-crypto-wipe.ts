// check-ui-ignore invalidate-all: a wipe clears a whole module out of the vault — wallets, capital,
// history and the protection level all read differently afterwards
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { textOf } from '@/shared/lib/notify'

const PREVIEW_KEY = ['crypto-wipe-preview']
const BACKUPS_KEY = ['vault-backups']

/**
 * The token the vault accepts as "yes, wipe it" (`WIPE_CONFIRM` in
 * `service/cryptotrack/wipe.rs`). The user types a word in their own language
 * and this is what travels, so neither an empty call nor a mis-click can wipe
 * anything.
 */
export const WIPE_TOKEN = 'WIPE'

/**
 * Settings · Crypto · Wiping: what a wipe would take, the vault's backups, and
 * the wipe itself.
 *
 * The backup commands are called here rather than through `vault-backups`:
 * two slices of one layer do not import each other (AGENTS.md rule 9).
 */
export function useCryptoWipe() {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  const preview = useQuery({
    queryKey: PREVIEW_KEY,
    queryFn: () => call<CryptoWipePreview>('crypto_wipe_preview'),
  })

  const backups = useQuery({
    queryKey: BACKUPS_KEY,
    queryFn: () => call<VaultBackup[]>('list_vault_backups'),
  })

  const makeBackup = useMutation({
    mutationFn: () => call<VaultBackup>('create_vault_backup'),
    onSettled: () => queryClient.invalidateQueries({ queryKey: BACKUPS_KEY }),
    meta: {
      success: (data) => ({ text: textOf('cryptoTrack.wipe.backupMade'), description: (data as VaultBackup).name }),
      error: 'cryptoTrack.wipe.backupFailed',
    },
  })

  const wipe = useMutation({
    mutationFn: () =>
      call<CryptoWipeReport>('crypto_wipe_module', { input: { confirm: WIPE_TOKEN } satisfies CryptoWipeInput }),
    // Everything on screen was counted with the module in it.
    onSettled: () => queryClient.invalidateQueries(),
    meta: {
      // No undo: the wipe cannot be taken back, only restored from the copy it made.
      success: (data) => ({
        text: textOf('cryptoTrack.wipe.done'),
        description: wipeOutcome(data as CryptoWipeReport),
      }),
      error: 'cryptoTrack.wipe.failed',
    },
  })

  /** What the wipe did, in one line: the wallets it archived and the copy it left. */
  function wipeOutcome(report: CryptoWipeReport): string {
    const archived = t('cryptoTrack.wipe.doneWallets', report.wallets.length)
    return report.backup ? `${archived} · ${t('cryptoTrack.wipe.doneBackup', { name: report.backup })}` : archived
  }

  return { preview, backups, makeBackup, wipe }
}
