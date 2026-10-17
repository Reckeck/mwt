import { onScopeDispose } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call, onEvent } from '@/shared/api/ipc'
import { notify, textOf } from '@/shared/lib/notify'

const LINK_KEY = ['vault-link']

/** The external copy of the unlocked vault: status, link, unlink. */
export function useVaultLink() {
  const queryClient = useQueryClient()
  const refresh = () => queryClient.invalidateQueries({ queryKey: LINK_KEY })

  const status = useQuery({
    queryKey: LINK_KEY,
    queryFn: () => call<VaultLinkStatus>('vault_link_status'),
  })

  // A copy that fails in the background (drive offline, grant revoked) says so.
  const stop = onEvent<string>('vault-mirror-failed', (reason) => {
    void refresh()
    notify.error(reason, 'vaultLink.copyFailed')
  })
  onScopeDispose(() => void stop.then((unlisten) => unlisten()))

  const link = useMutation({
    /** `existing`: replace a file the user picks; otherwise create a new one. */
    mutationFn: (existing: boolean) => call<VaultLinkStatus | null>('link_vault_external', { input: { existing } }),
    onSettled: () => refresh(),
    meta: {
      // A cancelled dialog returns null: nothing to report.
      success: (data) =>
        data
          ? { text: textOf('vaultLink.linked'), description: (data as VaultLinkStatus).display_name ?? undefined }
          : '',
      error: 'vaultLink.linkFailed',
    },
  })

  const unlink = useMutation({
    mutationFn: () => call('unlink_vault_external'),
    onSettled: () => refresh(),
    meta: { success: 'vaultLink.unlinked', error: 'vaultLink.unlinkFailed' },
  })

  return { status, link, unlink }
}
