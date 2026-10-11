<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowLeftRight } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'

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
const fmt = (value: string) => Number(value).toLocaleString(undefined, { maximumFractionDigits: 6 })
</script>

<template>
  <div class="grid items-start gap-4 @4xl:grid-cols-[1fr_22rem]">
    <Card>
      <template #header>
        <div class="flex flex-wrap items-center justify-between gap-2">
          <p class="font-medium">{{ t('rates.title', { base: (table.data.value?.base ?? '').toUpperCase() }) }}</p>
          <Input v-model="search" class="h-8 w-40" :placeholder="t('rates.search')" />
        </div>
      </template>
      <div class="max-h-[32rem] overflow-y-auto">
        <table class="w-full text-sm tabular-nums">
          <thead class="sticky top-0 bg-card text-xs text-muted-foreground">
            <tr>
              <th class="py-1 pr-2 text-left font-normal">{{ t('rates.currency') }}</th>
              <th class="px-2 py-1 text-right font-normal">{{ t('rates.inBase', { base: (table.data.value?.base ?? '').toUpperCase() }) }}</th>
              <th class="px-2 py-1 text-right font-normal">{{ t('rates.perBase', { base: (table.data.value?.base ?? '').toUpperCase() }) }}</th>
              <th class="py-1 pl-2 text-right font-normal">{{ t('rates.updated') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in rows" :key="row.currency" class="border-t border-border">
              <td class="py-1.5 pr-2 font-medium uppercase" :class="!row.tracked && 'font-normal text-muted-foreground'">{{ row.currency }}</td>
              <td class="px-2 py-1.5 text-right">{{ fmt(row.in_base) }}</td>
              <td class="px-2 py-1.5 text-right">{{ fmt(row.per_base) }}</td>
              <td class="py-1.5 pl-2 text-right text-xs text-muted-foreground">{{ row.updated_at ? formatDateTime(row.updated_at) : '—' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p class="mt-3 text-xs text-muted-foreground">{{ t('rates.hint') }}</p>
    </Card>

    <Card>
      <template #header>
        <p class="font-medium">{{ t('rates.converter') }}</p>
      </template>
      <div class="grid gap-3">
        <div class="grid grid-cols-[1fr_auto_1fr] items-end gap-2">
          <div class="grid gap-1.5">
            <Label for="convert-from">{{ t('rates.from') }}</Label>
            <Select id="convert-from" v-model="from">
              <option v-for="code in codes" :key="code" :value="code">{{ code.toUpperCase() }}</option>
            </Select>
          </div>
          <Button variant="ghost" size="icon" class="size-9" :aria-label="t('rates.swap')" @click="swap">
            <ArrowLeftRight class="size-4" />
          </Button>
          <div class="grid gap-1.5">
            <Label for="convert-to">{{ t('rates.to') }}</Label>
            <Select id="convert-to" v-model="to">
              <option v-for="code in codes" :key="code" :value="code">{{ code.toUpperCase() }}</option>
            </Select>
          </div>
        </div>
        <div class="grid grid-cols-2 gap-2">
          <div class="grid gap-1.5">
            <Label for="convert-amount">{{ t('rates.amount') }}</Label>
            <AmountInput id="convert-amount" v-model="amount" />
          </div>
          <div class="grid gap-1.5">
            <Label for="convert-date">{{ t('rates.date') }}</Label>
            <Input id="convert-date" v-model="date" type="date" />
          </div>
        </div>
        <p class="text-2xl font-semibold tabular-nums">
          {{ converted.data.value?.amount ? `${fmt(converted.data.value.amount)} ${to.toUpperCase()}` : '—' }}
        </p>
        <p v-if="converted.data.value && converted.data.value.amount === null" class="text-xs text-muted-foreground">{{ t('rates.noRate') }}</p>
      </div>
    </Card>
  </div>
</template>
