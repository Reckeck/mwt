// check-ui-ignore invalidate-all: a restore replaces the whole document, so any screen's data may have changed
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { textOf } from '@/shared/lib/notify'

const BACKUPS_KEY = ['vault-backups']

/** Backup copies of the open vault file: the list, one more now, and putting one back. */
export function useVaultBackups() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const refresh = () => queryClient.invalidateQueries({ queryKey: BACKUPS_KEY })

  const backups = useQuery({
    queryKey: BACKUPS_KEY,
    queryFn: () => call<VaultBackup[]>('list_vault_backups'),
  })

  const create = useMutation({
    mutationFn: () => call<VaultBackup>('create_vault_backup'),
    onSettled: () => refresh(),
    meta: {
      success: (data) => ({ text: textOf('vaultBackups.created'), description: (data as VaultBackup).name }),
      error: 'vaultBackups.createFailed',
    },
  })

  const restore = useMutation({
    mutationFn: (name: string) =>
      call<VaultBackup>('restore_vault_backup', { input: { name } satisfies RestoreVaultBackupInput }),
    // The whole document was replaced: nothing on screen can be trusted to be current.
    onSettled: () => queryClient.invalidateQueries(),
    meta: {
      success: (data) => ({
        text: textOf('vaultBackups.restored'),
        description: t('vaultBackups.restoredHint', { name: (data as VaultBackup).name }),
      }),
      error: 'vaultBackups.restoreFailed',
    },
  })

  return { backups, create, restore }
}
