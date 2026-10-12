<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { Money } from '@/entities/currency'
import { FormField } from '@/shared/ui/form-field'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { required, useFormValidation } from '@/shared/lib/form'
import { call, parseError } from '@/shared/api/ipc'
import { toIsoTimestamp, toLocalInput } from '@/shared/lib/datetime'
import { LEDGER_KEYS, PLANNING_KEYS, localDate } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { UtilityBillDialogProps } from './utility-bill.types'

/**
 * The month's utilities as one bill: meters get their reading typed in place
 * (the amount follows), fixed bills and rent come pre-filled, and one button
 * books every line as its own payment — the way they are actually paid, on
 * one day.
 */
const props = defineProps<UtilityBillDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ paid: [count: number] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const period = ref(props.period ?? localDate(new Date()).slice(0, 7))
watch(open, (isOpen) => {
  if (isOpen) period.value = props.period ?? localDate(new Date()).slice(0, 7)
})

const bill = useQuery({
  queryKey: computed(() => ['utility-bill', props.propertyId, period.value]),
  queryFn: () => call<UtilityBill>('get_utility_bill', { propertyId: props.propertyId, period: period.value }),
  enabled: open,
})

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const payable = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter(
    (node) => !node.store.archived && (node.store.kind === 'bank' || node.store.kind === 'cash'),
  ),
)

interface Row {
  line: UtilityBillLine
  include: boolean
  reading: string
  amount: string
}
const rows = ref<Row[]>([])
const storeId = ref('')
const when = ref('')

watch(
  () => bill.data.value,
  (data) => {
    if (!data) return
    rows.value = data.lines.map((line) => ({
      line,
      // Already paid this month: left out unless ticked again.
      include: Number(line.paid) === 0,
      reading: line.reading ?? '',
      amount: line.amount ?? '',
    }))
    when.value = toLocalInput(data.due_at)
    const own = data.lines.map((line) => line.item.store_id).find(Boolean)
    if (!storeId.value) {
      const everyday = payable.value.find((node) => node.store.purpose === 'everyday')
      storeId.value = own ?? everyday?.store.id ?? payable.value[0]?.store.id ?? ''
    }
  },
  { immediate: true },
)

const unitOf = (item: UtilityItem) => ('metered' in item.kind ? item.kind.metered.unit : '')

/** A typed reading re-prices its meter line: (reading − previous) × tariff. */
function onReading(row: Row, value: string | number) {
  row.reading = String(value)
  const kind = row.line.item.kind
  if (!('metered' in kind) || !row.reading || row.line.previous_reading === null) return
  const used = Number(row.reading) - Number(row.line.previous_reading)
  if (used >= 0) row.amount = (used * Number(kind.metered.tariff)).toFixed(2)
  row.include = true
}

const consumption = (row: Row) =>
  row.reading && row.line.previous_reading !== null ? Number(row.reading) - Number(row.line.previous_reading) : null

const totals = computed(() => {
  const sums = new Map<string, number>()
  for (const row of rows.value) {
    if (!row.include || !(Number(row.amount) > 0)) continue
    sums.set(row.line.item.currency, (sums.get(row.line.item.currency) ?? 0) + Number(row.amount))
  }
  return [...sums.entries()]
})
const chosen = computed(() => rows.value.filter((row) => row.include && Number(row.amount) > 0))
const needsStore = computed(() => chosen.value.some((row) => !row.line.item.store_id))
const form = useFormValidation(
  () => ({
    when: required(when.value),
    store: needsStore.value ? required(storeId.value, 'pick') : null,
    lines: !chosen.value.length
      ? t('validation.billLines')
      : rows.value.some((row) => row.include && !(Number(row.amount) > 0))
        ? t('validation.billAmounts')
        : null,
  }),
  { resetOn: open },
)

const pay = useMutation({
  mutationFn: () =>
    call<Transaction[]>('pay_utility_bill', {
      input: {
        property_id: props.propertyId,
        period: period.value,
        store_id: storeId.value,
        timestamp: toIsoTimestamp(when.value),
        lines: chosen.value.map((row) => ({
          item_id: row.line.item.id,
          amount: row.amount,
          reading: 'metered' in row.line.item.kind && row.reading && row.reading !== row.line.reading ? row.reading : null,
          reading_day: null,
        })),
      } satisfies PayUtilityBillInput,
    }),
  onSuccess: (saved) => {
    toast.success(t('utilityBill.paid', { count: saved.length }))
    for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['utility-trend'], ['utility-bill'], ['utilities']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    emit('paid', saved.length)
    open.value = false
  },
  onError: (err) => toast.error(t('utilityBill.failed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('utilityBill.title')" size="lg">
    <form id="utility-bill-form" class="@container grid gap-4" @submit.prevent="form.submit(pay.mutate, $event)">
      <div class="grid gap-3 @sm:grid-cols-2">
        <FormField for="bill-date" :label="t('utilityBill.date')" required :error="form.error('when')">
          <Input id="bill-date" v-model="when" type="datetime-local" />
          <p v-if="bill.data.value?.pay_day" class="text-xs text-muted-foreground">
            {{ t('utilityBill.payDay', { day: bill.data.value.pay_day }) }}
          </p>
        </FormField>
        <FormField for="bill-store" :label="t('utilities.store')" :required="needsStore" :error="form.error('store')">
          <Select id="bill-store" v-model="storeId">
            <option value="" disabled>{{ t('utilityBill.pickStore') }}</option>
            <option v-for="node in payable" :key="node.store.id" :value="node.store.id">{{ node.store.name }}</option>
          </Select>
        </FormField>
      </div>

      <p v-if="bill.isSuccess.value && !rows.length" class="text-sm text-muted-foreground">{{ t('utilities.empty') }}</p>

      <p v-if="form.error('lines')" class="text-xs text-destructive" role="alert">{{ form.error('lines') }}</p>
      <ul
        class="grid divide-y divide-border rounded-lg border"
        :class="form.error('lines') ? 'border-destructive' : 'border-border'"
        :data-invalid="form.error('lines') ? '' : undefined"
      >
        <li v-for="row in rows" :key="row.line.item.id" class="grid items-center gap-2 p-2.5 @sm:grid-cols-[1.5rem_1fr_9rem_9rem]">
          <input
            v-model="row.include"
            type="checkbox"
            class="size-4 accent-primary"
            :aria-label="t('utilityBill.include', { name: row.line.item.name })"
          />
          <div class="min-w-0">
            <p class="flex items-center gap-2 truncate text-sm font-medium">
              {{ row.line.item.name }}
              <Badge v-if="Number(row.line.paid) > 0" variant="secondary">
                {{ t('utilityBill.alreadyPaid') }} <Money :amount="row.line.paid" :currency="row.line.item.currency" :convert="false" />
              </Badge>
            </p>
            <p v-if="'metered' in row.line.item.kind" class="text-xs text-muted-foreground tabular-nums">
              {{ t('utilityBill.previous', { value: row.line.previous_reading ?? '—', unit: unitOf(row.line.item) }) }}
              <template v-if="consumption(row) !== null">
                · {{ t('utilities.consumption') }}: {{ consumption(row) }} {{ unitOf(row.line.item) }}
              </template>
            </p>
            <p v-if="'metered' in row.line.item.kind && !row.reading" class="text-xs text-muted-foreground">
              {{ t('utilityBill.enterReading') }}
            </p>
            <p v-else-if="row.line.includes_deposit" class="text-xs text-muted-foreground">{{ t('utilities.depositIncluded') }}</p>
          </div>
          <AmountInput
            v-if="'metered' in row.line.item.kind"
            :model-value="row.reading"
            :placeholder="t('utilities.reading')"
            :aria-label="t('utilities.reading')"
            @update:model-value="(value) => onReading(row, value)"
          />
          <span v-else class="hidden @sm:block" />
          <AmountInput v-model="row.amount" :decimals="2" :aria-label="t('utilities.amount')">
            <template #suffix>
              <span class="px-2 text-xs uppercase text-muted-foreground">{{ row.line.item.currency }}</span>
            </template>
          </AmountInput>
        </li>
      </ul>
    </form>
    <template #footer>
      <div class="mr-auto flex flex-wrap items-baseline gap-2 text-sm">
        <span class="text-muted-foreground">{{ t('utilityBill.total') }}</span>
        <span v-for="[currency, sum] in totals" :key="currency" class="font-semibold">
          <Money :amount="sum" :currency="currency" />
        </span>
      </div>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="utility-bill-form" :disabled="pay.isPending.value">
        {{ t('utilityBill.pay', { count: chosen.length }) }}
      </Button>
    </template>
  </Dialog>
</template>
