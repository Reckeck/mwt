<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Textarea } from '@/shared/ui/textarea'
import { call, parseError } from '@/shared/api/ipc'
import { assetKey, fiatRef } from '@/shared/lib/asset-ref'
import { keepOriginalMoment, toLocalInput } from '@/shared/lib/datetime'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { decimalSub, parseDecimal } from '@/shared/lib/decimal'
import { formatMoney } from '@/shared/lib/format'
import { amountClass, body, txId } from '@/shared/lib/transactions'
import { required, useFormValidation } from '@/shared/lib/form'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { AdjustmentDialogProps, AdjustmentRow } from './store-adjust.types'

const props = withDefaults(defineProps<AdjustmentDialogProps>(), { transaction: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const node = computed(() => flattenStoreTree(tree.data.value ?? []).find((entry) => entry.store.id === props.storeId))
/** Cash and bank accounts hold money only: a currency, not any asset. */
const fiatOnly = computed(() => ['cash', 'bank'].includes(node.value?.store.kind ?? ''))

const editing = computed(() => (props.transaction ? body(props.transaction, 'adjustment') : null))

const timestamp = ref('')
const comment = ref('')
const rows = ref<AdjustmentRow[]>([])

/** The moment saved: the edited correction keeps its seconds while the field is left as it was. */
const moment = computed(() => keepOriginalMoment(timestamp.value, editing.value?.meta.timestamp))

const draft = useQuery({
  queryKey: computed(() => ['adjustment-draft', props.storeId, timestamp.value, props.transaction ? txId(props.transaction) : null]),
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

const keyOf = assetKey

/** The rows the edited correction started with: kept on save even when they now match. */
const pinned = ref(new Set<string>())

// The ledger's balances fill the rows; what the user typed (or the edited
// correction held) stays put when the date moves the balances.
function fill(data: AdjustmentDraft) {
  const typed = new Map(rows.value.map((row) => [keyOf(row.asset), row]))
  const next: AdjustmentRow[] = data.lines.map((line) => ({
    asset: line.asset,
    before: line.before,
    actual: typed.get(keyOf(line.asset))?.actual ?? '',
  }))
  const known = new Set(next.map((row) => keyOf(row.asset)))
  for (const row of typed.values()) {
    if (!known.has(keyOf(row.asset))) next.push({ ...row, before: '0' })
  }
  if (!rows.value.length && editing.value) {
    for (const line of editing.value.lines) {
      const row = next.find((entry) => keyOf(entry.asset) === keyOf(line.asset))
      if (row) row.actual = line.after
      else next.push({ asset: line.asset, before: '0', actual: line.after })
    }
  }
  rows.value = next
}
watch(
  () => draft.data.value,
  (data) => {
    if (data && open.value) fill(data)
  },
)

// Every opening starts from the edited correction (or from nothing) and the
// ledger as it is now: the cached draft fills the rows at once, a fresh one
// follows. Without this a second opening showed no rows at all.
watch(
  open,
  (isOpen) => {
    if (!isOpen) return
    timestamp.value = editing.value ? toLocalInput(editing.value.meta.timestamp) : ''
    comment.value = editing.value?.meta.comment ?? ''
    rows.value = []
    pinned.value = new Set((editing.value?.lines ?? []).map((line) => keyOf(line.asset)))
    void nextTick(() => {
      if (draft.data.value) fill(draft.data.value)
      void draft.refetch()
    })
  },
  { immediate: true },
)

/** Actual minus ledger, exactly; `null` while the field is empty or not a number. */
const difference = (row: AdjustmentRow): string | null => (row.actual.trim() ? decimalSub(row.actual, row.before) : null)

const changed = computed(() => rows.value.filter((row) => (difference(row) ?? '0') !== '0'))
/** Sent on save: what changed, plus the edited correction's own rows, which keep pinning their balance. */
const sent = computed(() =>
  rows.value.filter((row) => changed.value.includes(row) || (pinned.value.has(keyOf(row.asset)) && row.actual.trim())),
)
const badRow = (row: { actual: string }) => Boolean(row.actual.trim()) && parseDecimal(row.actual) === null
const form = useFormValidation(
  () => ({
    timestamp: required(timestamp.value),
    rows: rows.value.some(badRow) ? t('validation.number') : changed.value.length ? null : t('adjustment.nothing'),
  }),
  { resetOn: open },
)
/** The asset code a balance is shown in: a fiat code gets its symbol, a ticker stays. */
const shown = (value: string | null, asset: AssetRef, signed = false) =>
  value === null ? '—' : formatMoney(value, { currency: asset.symbol.toLowerCase(), signed })

/* --- an asset the ledger does not know about yet --- */
const newCurrency = ref('')
const newAsset = ref<AssetRef | null>(null)
const newRowAsset = computed(() =>
  fiatOnly.value ? (newCurrency.value ? fiatRef(newCurrency.value) : null) : newAsset.value,
)
const adder = useFormValidation(() => ({
  asset:
    required(newRowAsset.value, 'pick') ??
    (rows.value.some((row) => newRowAsset.value && keyOf(row.asset) === keyOf(newRowAsset.value))
      ? t('validation.duplicateAsset')
      : null),
}))
function addRow() {
  const asset = newRowAsset.value
  if (!asset) return
  rows.value.push({ asset, before: '0', actual: '' })
  newCurrency.value = ''
  newAsset.value = null
  adder.reset()
}

function removeRow(index: number) {
  rows.value.splice(index, 1)
}

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
      lines: sent.value.map((row) => ({
        asset: {
          symbol: row.asset.symbol,
          name: row.asset.name,
          coingecko_id: row.asset.coingecko_id,
          chain: row.asset.chain,
          contract: row.asset.contract,
        },
        actual: parseDecimal(row.actual) ?? row.actual,
      })),
    }
    return props.transaction
      ? call<Transaction>('update_transaction', { input: { id: txId(props.transaction), transaction: input } })
      : call<Transaction>('add_transaction', { input })
  },
  onSuccess: () => {
    toast.success(t('adjustment.saved'))
    for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['adjustment-draft']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    open.value = false
  },
  onError: (err) => toast.error(t('adjustment.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const title = computed(() =>
  node.value ? t('adjustment.titleFor', { name: node.value.path.join(' › ') }) : t('adjustment.title'),
)
</script>

<template>
  <Dialog v-model:open="open" :title="title" :description="t('adjustment.hint')" size="lg">
    <form id="adjustment-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <FormField
        for="adjustment-date"
        :label="t('adjustment.date')"
        class="sm:max-w-64"
        required
        :error="form.error('timestamp')"
        :hint="t('adjustment.dateHint')"
      >
        <Input id="adjustment-date" v-model="timestamp" type="datetime-local" class="px-2" />
      </FormField>

      <div
        class="overflow-hidden rounded-lg border"
        :class="form.error('rows') ? 'border-destructive' : 'border-border'"
        :data-invalid="form.error('rows') ? '' : undefined"
        data-tour="adjustment-rows"
      >
        <div class="grid grid-cols-[minmax(0,1fr)_8rem_9rem_7rem_2rem] items-center gap-2 border-b border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
          <span>{{ t('adjustment.asset') }}</span>
          <span class="text-right">{{ t('adjustment.ledger') }}</span>
          <span>{{ t('adjustment.actual') }}</span>
          <span class="text-right">{{ t('adjustment.difference') }}</span>
          <span />
        </div>
        <p v-if="!rows.length && draft.isSuccess.value" class="px-3 py-4 text-sm text-muted-foreground">
          {{ t('adjustment.empty') }}
        </p>
        <div
          v-for="(row, index) in rows"
          :key="keyOf(row.asset)"
          class="grid grid-cols-[minmax(0,1fr)_8rem_9rem_7rem_2rem] items-center gap-2 border-b border-border px-3 py-1.5 last:border-b-0"
        >
          <span class="truncate text-sm font-medium" :title="row.asset.name">{{ row.asset.symbol }}</span>
          <span class="truncate text-right text-sm tabular-nums text-muted-foreground" :title="row.before">
            {{ shown(row.before, row.asset) }}
          </span>
          <AmountInput
            v-model="row.actual"
            :placeholder="row.before"
            :class="badRow(row) && 'border-destructive'"
            :aria-invalid="badRow(row)"
            :aria-label="t('adjustment.actualFor', { asset: row.asset.symbol })"
          />
          <span
            class="truncate text-right text-sm tabular-nums"
            :class="difference(row) === null ? 'text-muted-foreground' : amountClass(difference(row) ?? '0')"
          >
            {{ shown(difference(row), row.asset, true) }}
          </span>
          <Button
            v-if="row.before === '0'"
            type="button"
            variant="ghost"
            size="icon"
            class="size-7"
            :aria-label="t('actions.delete')"
            @click="removeRow(index)"
          >
            <Trash2 class="size-3.5" />
          </Button>
          <span v-else />
        </div>
      </div>

      <p v-if="form.error('rows')" class="-mt-2 text-xs text-destructive" role="alert">{{ form.error('rows') }}</p>

      <FormField :label="t('adjustment.addAsset')" :error="adder.error('asset')">
        <div class="flex items-start gap-2">
          <div class="min-w-0 flex-1">
            <CurrencyPicker v-if="fiatOnly" v-model="newCurrency" :kinds="['fiat']" label="" />
            <AssetInput v-else v-model="newAsset" label="" />
          </div>
          <Button type="button" variant="outline" @click="adder.submit(addRow, $event)">
            <Plus class="size-4" />
            {{ t('actions.add') }}
          </Button>
        </div>
      </FormField>

      <div class="grid gap-1.5">
        <Label for="adjustment-comment">{{ t('form.comment') }}</Label>
        <Textarea id="adjustment-comment" v-model="comment" rows="1" :placeholder="t('adjustment.commentHint')" />
      </div>

      <p v-if="!changed.length && !form.error('rows')" class="text-xs text-muted-foreground">{{ t('adjustment.nothing') }}</p>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="adjustment-form" type="submit" :disabled="save.isPending.value">
        {{ t('adjustment.submit', { count: changed.length }) }}
      </Button>
    </template>
  </Dialog>
</template>
