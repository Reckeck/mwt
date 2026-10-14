<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowLeftRight } from 'lucide-vue-next'

import { CurrencyPicker } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { type ColumnDef, DataTable } from '@/shared/ui/data-table'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'
import { formatDateTime, formatMoney, formatQuantity } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

const { t } = useI18n()

const table = useQuery({ queryKey: ['fx-table'], queryFn: () => call<FxTable>('get_fx_table') })
const search = ref('')
const rows = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return [...(table.data.value?.rows ?? [])]
    .filter((row) => !needle || row.currency.includes(needle))
    .sort((left, right) => Number(right.tracked) - Number(left.tracked) || left.currency.localeCompare(right.currency))
})
const codes = computed(() => [table.data.value?.base ?? 'usd', ...(table.data.value?.rows ?? []).map((row) => row.currency)])

/* --- converter A ↔ B, at today's rate or a chosen date --- */
const from = ref('')
const to = ref('')
const amount = ref('100')
const date = ref('')
watch(
  () => table.data.value,
  (data) => {
    if (!data || from.value) return
    from.value = data.base
    to.value = data.rows.find((row) => row.tracked)?.currency ?? data.rows[0]?.currency ?? 'usd'
  },
  { immediate: true },
)
const converted = useQuery({
  queryKey: computed(() => ['convert', from.value, to.value, amount.value, date.value]),
  queryFn: () =>
    call<ConvertedAmount>('convert_amount', {
      input: {
        from: from.value,
        to: to.value,
        amount: amount.value.trim() || '0',
        at: date.value ? new Date(`${date.value}T12:00`).toISOString() : null,
      },
    }),
  enabled: computed(() => Boolean(from.value && to.value && Number(amount.value) > 0)),
})
function swap() {
  ;[from.value, to.value] = [to.value, from.value]
}
const fmt = (value: string) => formatQuantity(value, 6)

/** Tracked currencies first, then by code; a header click sorts by that column. */
const columns = computed<ColumnDef<FxRow>[]>(() => {
  const base = (table.data.value?.base ?? '').toUpperCase()
  return [
    {
      id: 'currency',
      header: t('rates.currency'),
      accessorKey: 'currency',
      cell: ({ row }) =>
        h('span', { class: cn('uppercase', row.original.tracked ? 'font-medium' : 'text-muted-foreground') }, row.original.currency),
    },
    {
      id: 'inBase',
      header: t('rates.inBase', { base }),
      accessorFn: (row) => Number(row.in_base),
      cell: ({ row }) => fmt(row.original.in_base),
      meta: { align: 'right' },
    },
    {
      id: 'perBase',
      header: t('rates.perBase', { base }),
      accessorFn: (row) => Number(row.per_base),
      cell: ({ row }) => fmt(row.original.per_base),
      meta: { align: 'right' },
    },
    {
      id: 'updated',
      header: t('rates.updated'),
      accessorFn: (row) => row.updated_at ?? '',
      cell: ({ row }) => (row.original.updated_at ? formatDateTime(row.original.updated_at) : '—'),
      meta: { align: 'right', class: 'text-xs text-muted-foreground' },
    },
  ]
})
</script>

<template>
  <div class="grid items-start gap-4 @4xl:grid-cols-[1fr_22rem]">
    <Card>
      <template #header>
        <div class="flex flex-wrap items-center justify-between gap-2">
          <h3 class="heading-card">{{ t('rates.title', { base: (table.data.value?.base ?? '').toUpperCase() }) }}</h3>
          <Input v-model="search" class="h-8 w-40" :placeholder="t('rates.search')" />
        </div>
      </template>
      <div class="max-h-[32rem] overflow-y-auto">
        <DataTable sticky-header :columns="columns" :data="rows" :get-row-id="(row) => row.currency" :empty-text="t('rates.empty')" />
      </div>
      <p class="mt-3 text-xs text-muted-foreground">{{ t('rates.hint') }}</p>
    </Card>

    <Card>
      <template #header>
        <h3 class="heading-card">{{ t('rates.converter') }}</h3>
      </template>
      <div class="grid gap-3">
        <div class="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-end gap-2">
          <div class="grid gap-1.5">
            <Label for="convert-from">{{ t('rates.from') }}</Label>
            <CurrencyPicker id="convert-from" v-model="from" label="" compact :extra="codes" />
          </div>
          <Button variant="ghost" size="icon" class="size-9" :aria-label="t('rates.swap')" @click="swap">
            <ArrowLeftRight class="size-4" />
          </Button>
          <div class="grid gap-1.5">
            <Label for="convert-to">{{ t('rates.to') }}</Label>
            <CurrencyPicker id="convert-to" v-model="to" label="" compact :extra="codes" />
          </div>
        </div>
        <div class="grid grid-cols-2 gap-2 [&>*]:min-w-0">
          <div class="grid gap-1.5">
            <Label for="convert-amount">{{ t('rates.amount') }}</Label>
            <AmountInput id="convert-amount" v-model="amount" />
          </div>
          <div class="grid gap-1.5">
            <Label for="convert-date">{{ t('rates.date') }}</Label>
            <DateTimePicker id="convert-date" v-model="date" />
          </div>
        </div>
        <p class="figure">
          {{ converted.data.value?.amount ? formatMoney(converted.data.value.amount, { currency: to }) : '—' }}
        </p>
        <p v-if="converted.data.value && converted.data.value.amount === null" class="text-xs text-muted-foreground">{{ t('rates.noRate') }}</p>
      </div>
    </Card>
  </div>
</template>
