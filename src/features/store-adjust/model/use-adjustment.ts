import { type Ref, computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { assetKey, fiatRef } from '@/shared/lib/asset-ref'
import { keepOriginalMoment, toLocalInput } from '@/shared/lib/datetime'
import { formatMoney } from '@/shared/lib/format'
import { required, useFormValidation } from '@/shared/lib/form'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { body, txId } from '@/shared/lib/transactions'

import { adjustmentLines, badRow, difference, mergeDraftRows, shownBalance } from '../lib/adjustment-rows'
import type { AdjustmentDialogProps, AdjustmentRow } from '../store-adjust.types'
import { keepPreviousFor } from '@/shared/lib/query'
import { useStoreTree } from '@/shared/api/queries'
import { useSettingsStore } from '@/shared/store/settings'

/**
 * A balance correction: the ledger's balances at a moment, what is really
 * there, and the difference written as one operation. Every opening starts
 * from the edited correction (or from nothing) and the ledger as it is now.
 */
export function useAdjustment(props: AdjustmentDialogProps, open: Ref<boolean>) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settings = useSettingsStore()

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
    // an empty cash wallet or account still has a currency to type a balance in
    if (fiatOnly.value && !rows.value.length)
      rows.value.push({ asset: fiatRef(settings.baseCurrency), before: '0', actual: '' })
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
      // a new correction of money is for now: the short form has no date field
      timestamp.value = editing.value
        ? toLocalInput(editing.value.meta.timestamp)
        : fiatOnly.value
          ? toLocalInput(new Date().toISOString())
          : ''
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

  /** One currency, a new correction (Wallets-3 · 20): the short form without the table. */
  const simple = computed(() => fiatOnly.value && !editing.value && rows.value.length === 1)
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
      rows: rows.value.some(badRow)
        ? t('validation.number')
        : changed.value.length
          ? null
          : simple.value && !rows.value[0]?.actual.trim()
            ? required('')
            : t('adjustment.nothing'),
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

  /** «1 014,30 $ · 95 900 ₽»: the base sum first when the account holds another currency. */
  const inApp = computed(() => {
    const row = rows.value[0]
    if (!row) return ''
    const own = shownBalance(row.before, row.asset)
    const base = settings.baseCurrency
    if (row.asset.symbol.toLowerCase() === base.toLowerCase() || !node.value) return own
    return `${formatMoney(node.value.own_base, { currency: base, decimals: 2 })} · ${own}`
  })

  return { fiatOnly, simple, inApp, node, title, timestamp, comment, rows, draft, changed, form, save }
}
