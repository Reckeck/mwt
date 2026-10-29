import { useMutation } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { useVaultStore } from '@/shared/store/vault'

import type { ForgetTarget, ForgottenRow } from '../vault-forget.types'

/**
 * Take vaults out of the recent list. Only the list entry goes, never the file,
 * so there is no question: the toast offers `Undo` (`restoreRecent`).
 */
export function useVaultForget() {
  const { t } = useI18n()
  const vault = useVaultStore()

  const forget = useMutation({
    mutationFn: async (target: ForgetTarget): Promise<ForgottenRow> => ({
      path: target.path,
      index: await vault.forgetRecent(target.path),
    }),
    meta: {
      error: 'vaultForget.failed',
      success: (_data: unknown, target: unknown) => t('vaultForget.forgotten', { name: (target as ForgetTarget).name }),
      undo: (data: unknown) => {
        const row = data as ForgottenRow
        return vault.restoreRecent(row.path, row.index)
      },
    },
  })

  const clear = useMutation({
    mutationFn: async (targets: ForgetTarget[]): Promise<ForgottenRow[]> => {
      const rows: ForgottenRow[] = []
      for (const target of targets) rows.push({ path: target.path, index: await vault.forgetRecent(target.path) })
      return rows
    },
    meta: {
      error: 'vaultForget.failed',
      success: 'vaultForget.cleared',
      // Last removed comes back first, so every row finds its old index right.
      undo: async (data: unknown) => {
        for (const row of [...(data as ForgottenRow[])].reverse()) await vault.restoreRecent(row.path, row.index)
      },
    },
  })

  return { forget, clear }
}
