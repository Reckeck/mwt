<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { toIsoTimestamp, toLocalInput } from '@/shared/lib/datetime'
import { LEDGER_KEYS, PLANNING_KEYS, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import BillLineRow from './BillLineRow.vue'
import type { BillRow, UtilityBillDialogProps } from './utility-bill.types'
import { useStoreTree } from '@/shared/api/queries'

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
  // no placeholder: a watcher fills the bill rows from this month's data
  queryKey: computed(() => ['utility-bill', props.propertyId, period.value]),
  queryFn: () => call<UtilityBill>('get_utility_bill', { propertyId: props.propertyId, period: period.value }),
  enabled: open,
})

const tree = useStoreTree()
const payable = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter(
    (node) => !node.store.archived && (node.store.kind === 'bank' || node.store.kind === 'cash'),
  ),
)

const rows = ref<BillRow[]>([])
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
      zoneReadings: Object.fromEntries(line.zones.map((zone) => [zone.zone, zone.reading ?? ''])),
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

/** A multi-rate meter's readings typed here (not the ones already stored). */
const zoneReadings = (row: BillRow): ZoneReadingInput[] =>
  row.line.zones.flatMap((zone) => {
    const typed = row.zoneReadings[zone.zone]
    return typed && typed !== zone.reading ? [{ zone: zone.zone, reading: typed }] : []
  })

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
          zone_readings: zoneReadings(row),
        })),
      } satisfies PayUtilityBillInput,
    }),
  onSuccess: (saved) => {
    for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['utility-trend'], ['utility-bill'], ['utilities']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    emit('paid', saved.length)
    open.value = false
  },
  meta: {
    error: 'utilityBill.failed',
    success: (data) => t('utilityBill.paid', { count: (data as Transaction[]).length }),
  },
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('utilityBill.title')" size="lg">
    <form id="utility-bill-form" class="@container grid gap-4" @submit.prevent="form.submit(pay.mutate, $event)">
      <div class="grid gap-3 @xl:grid-cols-2">
        <FormField for="bill-date" :label="t('utilityBill.date')" required :error="form.error('when')">
          <DateTimePicker id="bill-date" v-model="when" mode="datetime" />
          <p v-if="bill.data.value?.pay_day" class="text-xs text-muted-foreground">
            {{ t('utilityBill.payDay', { day: bill.data.value.pay_day }) }}
          </p>
        </FormField>
        <FormField for="bill-store" :label="t('utilities.store')" :required="needsStore" :error="form.error('store')">
          <StoreNodeSelect
            id="bill-store"
            v-model="storeId"
            :nodes="payable"
            flat
            :placeholder="t('utilityBill.pickStore')"
            :invalid="Boolean(form.error('store'))"
          />
        </FormField>
      </div>

      <p v-if="bill.isSuccess.value && !rows.length" class="text-sm text-muted-foreground">{{ t('utilities.empty') }}</p>

      <p v-if="form.error('lines')" class="text-xs text-destructive" role="alert">{{ form.error('lines') }}</p>
      <ul
        class="grid divide-y divide-border rounded-lg border"
        :class="form.error('lines') ? 'border-destructive' : 'border-border'"
        :data-invalid="form.error('lines') ? '' : undefined"
      >
        <BillLineRow v-for="row in rows" :key="row.line.item.id" :row="row" />
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
