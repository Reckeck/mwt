import { useMutation } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { useVaultStore } from '@/shared/store/vault'

/**
 * «Найти файл…» for a history row whose file is gone: the system dialog picks
 * the file, the row points at it (same place in the list). Resolves to the new
 * path, or `null` when the dialog was closed.
 */
export function useVaultRelocate() {
  const { t } = useI18n()
  const vault = useVaultStore()

  return useMutation({
    mutationFn: async (path: string): Promise<string | null> => {
      const picked = await vault.pickVaultFile(false)
      if (!picked) return null
      await vault.relocateRecent(path, picked.path)
      return picked.path
    },
    meta: {
      error: 'vaultForget.relocateFailed',
      success: (data: unknown) => (data ? t('vaultForget.relocated') : ''),
    },
  })
}
