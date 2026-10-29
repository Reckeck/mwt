import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { pluralForm } from '@/shared/lib/format'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/** Every query an operation can show up in. */
const STALE_KEYS: ReadonlyArray<readonly string[]> = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances']]

export interface BulkCategory {
  ids: string[]
  category: string
  /** For the toast: `Category `Transfer` changed on 2 operations`. */
  name: string
}

export interface BulkMove {
  ids: string[]
  storeId: string
  name: string
}

/**
 * What the bar does with the picked operations (Ops-1 · 4): no `Are you sure?`,
 * each change comes back with `Undo` in its toast.
 */
export function useTransactionBulk(onDone: () => void) {
  const { t, locale } = useI18n()
  const queryClient = useQueryClient()
  const refresh = () => {
    for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
    onDone()
  }
  const counted = (key: string, count: number, values: Record<string, unknown> = {}) =>
    t(`${key}.${pluralForm(count, locale.value)}`, { count, ...values })

  const remove = useMutation({
    mutationFn: (ids: string[]) => call<DeletedTransaction[]>('delete_transactions', { ids }),
    onSuccess: refresh,
    meta: {
      error: 'bulk.deleteFailed',
      success: (_data, ids) => counted('bulk.deleted', (ids as string[]).length),
      undo: (data) => call('restore_transactions', { items: data as DeletedTransaction[] }),
    },
  })

  const categorize = useMutation({
    mutationFn: (change: BulkCategory) =>
      call<TxCategories[]>('set_transaction_categories', {
        changes: change.ids.map((id) => ({ id, categories: [change.category] })),
      }),
    onSuccess: refresh,
    meta: {
      error: 'bulk.categoryFailed',
      success: (_data, change) => {
        const { ids, name } = change as BulkCategory
        return counted('bulk.categorized', ids.length, { name })
      },
      undo: (data) => call('set_transaction_categories', { changes: data as TxCategories[] }),
    },
  })

  const move = useMutation({
    mutationFn: (change: BulkMove) =>
      call<TxStore[]>('move_transactions', {
        changes: change.ids.map((id) => ({ id, store_id: change.storeId })),
      }),
    onSuccess: refresh,
    meta: {
      error: 'bulk.moveFailed',
      success: (_data, change) => {
        const { ids, name } = change as BulkMove
        return counted('bulk.moved', ids.length, { name })
      },
      undo: (data) => call('move_transactions', { changes: data as TxStore[] }),
    },
  })

  return { remove, categorize, move }
}
