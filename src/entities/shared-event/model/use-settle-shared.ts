import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { i18n } from '@/shared/config/i18n'
import { formatDayMonthShort, formatMoney } from '@/shared/lib/format'
import type { NoticeText } from '@/shared/lib/notify'

import { lastSettlementId } from '../lib/settlement'

import type { DueChange, RemainderChange, SettleChange } from './settle.types'
import { useSharedEventSync } from './use-shared-event-sync'

const translate = (key: string, params?: Record<string, unknown>) => i18n.global.t(key, params ?? {})

/** `Undo` for anything that added a settlement: delete the one the answer ends with. */
function undoLast(eventId: string) {
  return (data: unknown) => {
    const settlementId = lastSettlementId(data as SharedEventView)
    if (!settlementId) return undefined
    return call<SharedEventView>('delete_shared_settlement', {
      input: { event_id: eventId, settlement_id: settlementId },
    })
  }
}

/** `received` / `Gave` / `Pay my share` / `Set off`: a transfer with a wallet, or a set-off without one. */
export function useSettleShared() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: SettleChange) => call<SharedEventView>('settle_shared', { input: change.input }),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.settle.failed',
      success: (_data, variables): NoticeText => {
        const change = variables as SettleChange
        // money: plain — the toast is text
        const amount = formatMoney(change.input.amount, { currency: change.currency, decimals: change.decimals })
        return {
          text: translate(`sharedEvents.settle.done.${change.kind}`, { name: change.name, amount }),
          description: change.walletName
            ? translate('sharedEvents.settle.doneWallet', { wallet: change.walletName })
            : undefined,
        }
      },
      undo: (data, variables) => undoLast((variables as SettleChange).input.event_id)(data),
    },
  })
}

/** `Write off the remainder` (my expense) and `Move onto the others` (shared among the rest). */
export function useSettleRemainder() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: RemainderChange) =>
      call<SharedEventView>(change.kind === 'write_off' ? 'write_off_shared' : 'reassign_shared', {
        input: change.input,
      }),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.remainder.failed',
      success: (_data, variables) => {
        const change = variables as RemainderChange
        // money: plain — the toast is text
        const amount = formatMoney(change.amount, { currency: change.currency, decimals: change.decimals })
        return translate(`sharedEvents.remainder.done.${change.kind}`, { name: change.name, amount })
      },
      undo: (data, variables) => undoLast((variables as RemainderChange).input.event_id)(data),
    },
  })
}

/** `Due date`: the term of the person's debt (the contact's usual debt term). */
export function useSharedDue() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: DueChange) => call<DebtTerms | null>('set_shared_due', { input: change.input }),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.due.failed',
      success: (_data, variables) => {
        const date = (variables as DueChange).input.due_at
        return date
          ? translate('sharedEvents.due.set', { date: formatDayMonthShort(date) })
          : translate('sharedEvents.due.cleared')
      },
      undo: (_data, variables) => {
        const change = variables as DueChange
        return call<DebtTerms | null>('set_shared_due', { input: { ...change.input, due_at: change.previous } })
      },
    },
  })
}
