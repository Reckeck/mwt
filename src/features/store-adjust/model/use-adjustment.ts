import { type Ref, computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { keepOriginalMoment, toLocalInput } from '@/shared/lib/datetime'
import { required, useFormValidation } from '@/shared/lib/form'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { body, txId } from '@/shared/lib/transactions'

import { adjustmentLines, badRow, difference, mergeDraftRows } from '../lib/adjustment-rows'
import type { AdjustmentDialogProps, AdjustmentRow } from '../store-adjust.types'
import { keepPreviousFor } from '@/shared/lib/query'
import { useStoreTree } from '@/shared/api/queries'

/**
 * A balance correction: the ledger's balances at a moment, what is really
 * there, and the difference written as one operation. Every opening starts
 * from the edited correction (or from nothing) and the ledger as it is now.
 */
export function useAdjustment(props: AdjustmentDialogProps, open: Ref<boolean>) {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  const tree = useStoreTree()
  const node = computed(() => flattenStoreTree(tree.data.value ?? []).find((entry) => entry.store.id === props.storeId))
  /** Cash and bank accounts hold money only: a currency, not any asset. */
  const fiatOnly = computed(() => ['cash', 'bank'].includes(node.value?.store.kind ?? ''))
  const title = computed(() =>
    node.value ? t('adjustment.titleFor', { name: node.value.path.join(' › ') }) : t('adjustment.title'),
  )

  const editing = computed(() => (props.transaction ? body(props.transaction, 'adjustment') : null))
  const timestamp = ref('')
  const comment = ref('')
  const rows = ref<AdjustmentRow[]>([])
  /** The moment saved: the edited correction keeps its seconds while the field is left as it was. */
  const moment = computed(() => keepOriginalMoment(timestamp.value, editing.value?.meta.timestamp))

  const draft = useQuery({
    queryKey: computed(() => [
      'adjustment-draft',
      props.storeId,
      timestamp.value,
      props.transaction ? txId(props.transaction) : null,
    ]),
    placeholderData: keepPreviousFor(() => props.storeId),
    queryFn: () =>
      call<AdjustmentDraft>('get_adjustment_draft', {
        input: {
          store_id: props.storeId,
          at: moment.value,
          replaces: props.transaction ? txId(props.transaction) : null,
        } satisfies AdjustmentDraftInput,
      }),
    enabled: open,
  })

  /** The rows the edited correction started with: kept on save even when they now match. */
  const pinned = ref(new Set<string>())
  const fill = (data: AdjustmentDraft) => {
    rows.value = mergeDraftRows(data, rows.value, editing.value)
  }
  watch(
    () => draft.data.value,
    (data) => {
      if (data && open.value) fill(data)
    },
  )
  // The cached draft fills the rows at once, a fresh one follows. Without this
  // a second opening showed no rows at all.
  watch(
    open,
    (isOpen) => {
      if (!isOpen) return
      timestamp.value = editing.value ? toLocalInput(editing.value.meta.timestamp) : ''
      comment.value = editing.value?.meta.comment ?? ''
      rows.value = []
      pinned.value = new Set((editing.value?.lines ?? []).map((line) => assetKey(line.asset)))
      void nextTick(() => {
        if (draft.data.value) fill(draft.data.value)
        void draft.refetch()
      })
    },
    { immediate: true },
  )

  const changed = computed(() => rows.value.filter((row) => (difference(row) ?? '0') !== '0'))
  /** Sent on save: what changed, plus the edited correction's own rows, which keep pinning their balance. */
  const sent = computed(() =>
    rows.value.filter(
      (row) => changed.value.includes(row) || (pinned.value.has(assetKey(row.asset)) && row.actual.trim()),
    ),
  )
  const form = useFormValidation(
    () => ({
      timestamp: required(timestamp.value),
      rows: rows.value.some(badRow) ? t('validation.number') : changed.value.length ? null : t('adjustment.nothing'),
    }),
    { resetOn: open },
  )

  const save = useMutation({
    mutationFn: () => {
      const input: TransactionInput = {
        type: 'adjustment',
        common: {
          timestamp: moment.value,
          comment: comment.value.trim(),
          categories: [],
          flow_id: null,
          contact_id: null,
          price_usd: null,
          fx_override: null,
          price: null,
          price_currency: null,
        },
        store_id: props.storeId,
        lines: adjustmentLines(sent.value),
      }
      return props.transaction
        ? call<Transaction>('update_transaction', { input: { id: txId(props.transaction), transaction: input } })
        : call<Transaction>('add_transaction', { input })
    },
    onSuccess: () => {
      for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['adjustment-draft']]) {
        void queryClient.invalidateQueries({ queryKey })
      }
      open.value = false
    },
    meta: { error: 'adjustment.saveFailed', success: 'adjustment.saved' },
  })

  return { fiatOnly, title, timestamp, comment, rows, draft, changed, form, save }
}
