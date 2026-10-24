import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { formatDate, formatDateTime } from '@/shared/lib/format'
import { noticeReminderId, noticeRoute } from '@/shared/lib/notice-link'
import { notify } from '@/shared/lib/notify'
import { openQuickAdd } from '@/shared/lib/quick-add'
import { rejectText } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

import { PLANNING_AND_LEDGER } from './use-cache-events'

/**
 * Notices from the background with something to do right away: a rejected
 * automation (retry / skip), skipped runs, and every notice the bell gets
 * (open it, mark a reminder done, put it off). These toasts carry two buttons
 * and stay longer, so they use `toast` directly rather than `notify`.
 */
export function useBackendNotices() {
  const { t } = useI18n()
  const router = useRouter()
  const queryClient = useQueryClient()
  const refreshPlanning = () => {
    for (const queryKey of PLANNING_AND_LEDGER) void queryClient.invalidateQueries({ queryKey })
  }

  useBackendEvent<AutomationRejectedEvent>('automation-rejected', (payload) => {
    refreshPlanning()
    const nodes = flattenStoreTree(queryClient.getQueryData<StoreTreeNode[]>(STORE_TREE_QUERY_KEY) ?? [])
    const storeName = (id: string) => nodes.find((node) => node.store.id === id)?.path.join(' › ') ?? id
    const settle = (cmd: 'retry_automation_run' | 'skip_automation_run') =>
      call(cmd, { runId: payload.run_id })
        .then(() => {
          refreshPlanning()
          notify.success(t(cmd === 'retry_automation_run' ? 'automation.run.retried' : 'automation.run.skipped'))
        })
        .catch((err) => notify.error(err, 'automation.run.retryFailed'))
    toast.error(t('automation.event.rejected', { name: payload.name, date: formatDate(payload.planned_at) }), {
      description: rejectText(payload.reason, t, storeName),
      duration: 30_000,
      action: { label: t('automation.run.retry'), onClick: () => void settle('retry_automation_run') },
      cancel: { label: t('automation.run.skip'), onClick: () => void settle('skip_automation_run') },
    })
  })

  useBackendEvent<{ count: number }>('automation-skipped', (payload) => {
    refreshPlanning()
    notify.warning(t('automation.event.skipped', { count: payload.count }))
  })

  useBackendEvent<NoticeRecord>('notification', async (record) => {
    void queryClient.invalidateQueries({ queryKey: ['notifications'] })
    if (record.key.startsWith('automation-rejected:')) return
    // The card opens over the screen the user is on when they click.
    const targetNow = () => noticeRoute(record.link, router.currentRoute.value.query)
    const reminderId = noticeReminderId(record.key)
    const settle = (cmd: 'set_reminder_done' | 'snooze_reminder') => {
      const until = new Date(Date.now() + 10 * 60_000).toISOString()
      const args = cmd === 'set_reminder_done' ? { id: reminderId, done: true } : { input: { id: reminderId, until } }
      void call(cmd, args)
        .then(() => {
          void queryClient.invalidateQueries({ queryKey: ['reminders'] })
          if (cmd === 'set_reminder_done') notify.success('reminder.done')
          else notify.success(t('reminder.snoozed', { when: formatDateTime(until) }))
        })
        .catch((err) => notify.error(err, 'reminder.saveFailed'))
    }
    // «Оплатить» (Auto-2 · 10) opens the payment form filled in; «Выполнено» stays in the bell and the list.
    const reminders = reminderId
      ? await queryClient
          .ensureQueryData({ queryKey: ['reminders'], queryFn: () => call<Reminder[]>('list_reminders') })
          .catch(() => [])
      : []
    const pay = reminders.find((entry) => entry.id === reminderId)?.pay
    const payNow = () =>
      openQuickAdd({ kind: 'payment', category: pay?.category ?? undefined, storeId: pay?.store_id ?? undefined })
    toast.info(record.title, {
      description: record.body,
      duration: reminderId ? 60_000 : 8_000,
      action: pay
        ? { label: t('reminder.payAction'), onClick: payNow }
        : reminderId
          ? { label: t('reminder.done'), onClick: () => settle('set_reminder_done') }
          : targetNow()
            ? { label: t('actions.open'), onClick: () => void router.push(targetNow() ?? '/') }
            : undefined,
      cancel: reminderId ? { label: t('reminder.snooze.10m'), onClick: () => settle('snooze_reminder') } : undefined,
    })
  })
}
