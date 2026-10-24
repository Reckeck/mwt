import { useMutation } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { pluralForm } from '@/shared/lib/format'
import { useVaultStore } from '@/shared/store/vault'

/**
 * A vault in the app's own folder: erase the file with its backup copies
 * (`delete_vault_file`), or hand a copy over first (`export_vault_copy`, the
 * system dialog picks the place — this side never names a path). The open
 * vault is refused by the backend, the toast shows its reason.
 */
export function useVaultDelete() {
  const { t, locale } = useI18n()
  const vault = useVaultStore()

  const erase = useMutation({
    mutationFn: async (path: string): Promise<DeleteVaultFileOutput> => {
      const done = await call<DeleteVaultFileOutput>('delete_vault_file', { input: { path } })
      await vault.loadRecents()
      return done
    },
    meta: {
      error: 'vaultDelete.failed',
      success: (data: unknown) => {
        const { file_name, backups } = data as DeleteVaultFileOutput
        if (!backups.length) return t('vaultDelete.deleted', { name: file_name })
        const count = backups.length
        return t(`vaultDelete.deletedWithCopies.${pluralForm(count, String(locale.value))}`, { name: file_name, count })
      },
    },
  })

  const saveCopy = useMutation({
    mutationFn: (path: string) => call<string | null>('export_vault_copy', { input: { path } }),
    meta: {
      error: 'vaultDelete.copyFailed',
      // `null`: the save dialog was closed, nothing to report.
      success: (data: unknown) => (data ? t('vaultDelete.copySaved', { where: String(data) }) : ''),
    },
  })

  return { erase, saveCopy }
}
