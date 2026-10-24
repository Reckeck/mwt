// check-ui-ignore invalidate-all: restoring or replacing the vault from sync changes every screen
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'

import { SYNC_HISTORY_KEY, SYNC_JOURNAL_KEY, SYNC_PEERS_KEY, SYNC_STATUS_KEY } from './use-sync-status'

/** What the user does with sync: settings, pairing, the list of changes, devices, undo. */
export function useSyncActions() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const refresh = (...keys: ReadonlyArray<readonly unknown[]>) => {
    for (const queryKey of keys) void queryClient.invalidateQueries({ queryKey })
  }

  const settings = useMutation({
    mutationFn: (input: Partial<SyncSettingsInput>) =>
      call<SyncStatus>('sync_set_settings', {
        input: {
          enabled: input.enabled ?? null,
          confirm_first: input.confirm_first ?? null,
          prefer_main: input.prefer_main ?? null,
          device_name: input.device_name ?? null,
          background_lock_min: input.background_lock_min ?? null,
        } satisfies SyncSettingsInput,
      }),
    onSuccess: (status) => queryClient.setQueryData(SYNC_STATUS_KEY, status),
    meta: {
      success: (_data, input) => {
        const enabled = (input as Partial<SyncSettingsInput>).enabled
        if (enabled === true) return t('sync.turnedOn')
        if (enabled === false) return t('sync.turnedOff')
        return t('settings.saved')
      },
      error: 'sync.saveFailed',
    },
  })

  const pair = useMutation({
    mutationFn: (sessionId: string) => call('sync_pair', { sessionId }),
    // The code dialog is the answer: nothing to say unless it failed.
    meta: { success: '', error: 'sync.pairFailed' },
  })

  const dismiss = useMutation({
    mutationFn: (sessionId: string) => call('sync_dismiss_offer', { sessionId }),
    // The banner just goes away: nothing to say unless it failed.
    meta: { success: '', error: 'sync.pairFailed' },
  })

  const confirmPairing = useMutation({
    mutationFn: (input: SyncDecisionInput) => call('sync_confirm_pairing', { input }),
    meta: {
      success: (_data, input) =>
        (input as SyncDecisionInput).accept ? t('sync.pairing.waiting') : t('sync.pairing.declined'),
      error: 'sync.pairFailed',
    },
  })

  const decidePlan = useMutation({
    mutationFn: (input: SyncDecisionInput) => call('sync_decide_plan', { input }),
    // Accepted or declined, the journal has a new line (the backend writes it when it is done).
    onSettled: () => refresh(SYNC_JOURNAL_KEY, SYNC_HISTORY_KEY),
    meta: {
      // Applying reports itself through `vault-synced` (with «Отменить»).
      success: (_data, input) => ((input as SyncDecisionInput).accept ? '' : t('sync.plan.declined')),
      error: 'sync.applyFailed',
    },
  })

  const forget = useMutation({
    mutationFn: (deviceId: string) => call('sync_forget_peer', { deviceId }),
    onSettled: () => refresh(SYNC_PEERS_KEY, SYNC_STATUS_KEY),
    meta: { success: 'sync.devices.forgotten', error: 'sync.devices.forgetFailed' },
  })

  const undo = useMutation({
    mutationFn: (batchId: string) => call<UndoReport>('sync_undo', { batchId }),
    onSettled: () => {
      refresh(SYNC_HISTORY_KEY, SYNC_JOURNAL_KEY)
      void queryClient.invalidateQueries()
    },
    meta: {
      success: (data) => {
        const report = data as UndoReport
        return {
          text: t('sync.undo.done', { count: report.restored }),
          description: report.skipped.length ? t('sync.undo.skipped', { names: report.skipped.join(', ') }) : undefined,
        }
      },
      error: 'sync.undo.failed',
    },
  })

  const restore = useMutation({
    mutationFn: () => call('sync_restore_backup'),
    onSettled: () => void queryClient.invalidateQueries(),
    meta: { success: 'sync.backup.restored', error: 'sync.backup.restoreFailed' },
  })

  // «Открыть порт»: the system asks for its password; the warning goes once it is done.
  const openFirewall = useMutation({
    mutationFn: () => call<SyncStatus>('sync_open_firewall'),
    onSuccess: (status) => queryClient.setQueryData(SYNC_STATUS_KEY, status),
    meta: { success: 'sync.firewall.opened', error: 'sync.firewall.openFailed' },
  })

  // A Flatpak cannot reach the host firewall: the user runs the command.
  const copyCommand = useMutation({
    mutationFn: (command: string) => navigator.clipboard.writeText(command),
    meta: { success: 'sync.firewall.copied', error: 'sync.firewall.copyFailed' },
  })

  return { settings, pair, dismiss, confirmPairing, decidePlan, forget, undo, restore, openFirewall, copyCommand }
}
