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
import { toIsoTimestamp } from '@/shared/lib/datetime'
import { formatMonthKey, pluralForm } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { LEDGER_KEYS, PLANNING_KEYS, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import BillLineRow from './BillLineRow.vue'
import { automated } from './model/line-note'
import type { BillRow, UtilityBillDialogProps } from './utility-bill.types'
import { useStoreTree } from '@/shared/api/queries'

/**
 * «Счёт за октябрь · Квартира» (Wallets-2 · 13): meters get their reading
 * typed in place (the amount follows), fixed bills come pre-filled, the rent an
 * automation pays stays off, and one button books every ticked line as its own
 * payment — the way they are actually paid, on one day.
 */
const props = defineProps<UtilityBillDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ paid: [count: number] }>()

const { t, locale } = useI18n()
const phone = useIsPhone()
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
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
const property = computed(() => nodes.value.find((node) => node.store.id === props.propertyId)?.store.name ?? '')
const title = computed(() => t('utilityBill.title', { month: formatMonthKey(period.value, { month: 'long' }), name: property.value }))
const automations = useQuery({
  queryKey: ['automations'],
  queryFn: () => call<Automation[]>('list_automations'),
  enabled: open,
})
/** The automation that pays a rent line, by name: «платит автоматизация «Аренда» 10-го». */
const automationName = (row: BillRow) => {
  const kind = row.line.item.kind
  const id = 'housing' in kind ? kind.housing.automation_id : null
  return automations.data.value?.find((entry) => entry.id === id)?.name ?? null
}
const payable = computed(() =>
  nodes.value.filter(
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
    rows.value = data.lines.map((line) => {
      const row: BillRow = {
        line,
        include: true,
        reading: line.reading ?? '',
        zoneReadings: Object.fromEntries(line.zones.map((zone) => [zone.zone, zone.reading ?? ''])),
        amount: line.amount ? Number(line.amount).toFixed(2) : '',
      }
      // Already paid this month, or paid by an automation: left out unless ticked again.
      row.include = Number(line.paid) === 0 && !automated(row)
      return row
    })
    when.value = localDate(new Date())
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
        timestamp: toIsoTimestamp(`${when.value}T${new Date().toTimeString().slice(0, 5)}`),
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
  <Dialog v-model:open="open" :title="title" :description="t('utilityBill.intro', { name: property })" size="bill">
    <form id="utility-bill-form" class="@container flex flex-col gap-4" data-bill-form @submit.prevent="form.submit(pay.mutate, $event)">
      <div class="grid gap-3 @lg:grid-cols-2">
        <FormField for="bill-date" :label="t('utilityBill.date')" required :error="form.error('when')">
          <DateTimePicker id="bill-date" v-model="when" class="w-full" :invalid="Boolean(form.error('when'))" />
          <p v-if="bill.data.value?.pay_day" class="text-xs text-muted-foreground">
            {{ t('utilityBill.payDay', { day: bill.data.value.pay_day }) }}
          </p>
        </FormField>
        <FormField for="bill-store" :label="t('utilityBill.store')" :required="needsStore" :error="form.error('store')">
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
      <ul class="flex flex-col" :data-invalid="form.error('lines') ? '' : undefined">
        <BillLineRow v-for="row in rows" :key="row.line.item.id" :row="row" :automation="automationName(row)" />
      </ul>
      <div class="flex items-baseline gap-2">
        <span class="text-sm text-muted-foreground">{{ t('utilityBill.total') }}</span>
        <span class="ms-auto flex flex-wrap justify-end gap-x-3 text-lg font-semibold tabular-nums" data-bill-total>
          <Money v-for="[currency, sum] in totals" :key="currency" :amount="sum" :currency="currency" />
          <template v-if="!totals.length">—</template>
        </span>
      </div>
    </form>
    <template #footer>
      <Button v-if="!phone" variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="utility-bill-form" :class="phone && 'h-11 w-full'" :disabled="pay.isPending.value">
        {{ t(`utilityBill.pay.${pluralForm(chosen.length, locale)}`, { count: chosen.length }) }}
      </Button>
    </template>
  </Dialog>
</template>
