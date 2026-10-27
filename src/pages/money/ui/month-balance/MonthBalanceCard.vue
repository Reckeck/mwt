<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CarryOverSwitch } from '@/features/month-carry-over'
import { Money } from '@/entities/currency'
import { formatMonthKey } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { SpendingMonths } from '../spending'
import MonthCarryLine from './MonthCarryLine.vue'

/**
 * «По месяцам» of the month's balance (Money 3 · 15): the bars, the carry
 * switch, and the table of what came, went, was left and carried. A month is
 * picked with a click on its bar. On a phone the months are a list.
 */
const props = defineProps<{ balance: MonthBalance; month: string }>()
const emit = defineEmits<{ pick: [month: string] }>()

const { t } = useI18n()
const short = (key: string) => formatMonthKey(key, { month: 'short' }).replace('.', '')
const carries = computed(() => props.balance.carry_since !== null)
const tone = (value: string) => (Number(value) < 0 ? 'text-warning' : 'text-success')
const latestFirst = computed(() => [...props.balance.rows].reverse())

const LINES = computed(() => [
  { key: 'income' as const, label: t('balance.income'), signed: true, class: '' },
  { key: 'spent' as const, label: t('balance.spent'), signed: false, class: '' },
  { key: 'left' as const, label: t('balance.left'), signed: true, class: 'font-semibold' },
  ...(carries.value ? [{ key: 'total' as const, label: t('balance.carried'), signed: false, class: 'font-semibold' }] : []),
])
const cell = (row: MonthBalanceRow, key: 'income' | 'spent' | 'left' | 'total') =>
  key === 'spent' ? `-${row.spent}`.replace('--', '') : row[key]

/** The table worked out once: a head per month, a cell per line and month. */
const heads = computed(() =>
  props.balance.rows.map((row) => ({
    month: row.month,
    picked: row.month === props.month,
    label: row.current ? `${short(row.month)} · ${t('balance.running')}` : short(row.month),
  })),
)
const table = computed(() =>
  LINES.value.map((line) => ({
    ...line,
    cells: props.balance.rows.map((row) => ({
      month: row.month,
      amount: cell(row, line.key),
      class: cn(line.key === 'left' && tone(row.left), row.month === props.month && 'bg-accent/50'),
    })),
  })),
)
</script>

<template>
  <section class="grid gap-3 rounded-xl border bg-card px-4 py-3 shadow-sm" data-tour="balance-months">
    <div class="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm">
      <b>{{ t('money.months.title') }}</b>
      <span class="text-muted-foreground max-md:hidden">{{ t('balance.pickHint') }}</span>
      <span class="flex-1" />
      <CarryOverSwitch :carry-since="props.balance.carry_since" :from="props.balance.rows[0]?.month ?? props.month" />
    </div>
    <MonthCarryLine v-if="carries" class="max-md:hidden" :balance="props.balance" @pick="emit('pick', $event)" />
    <SpendingMonths :class="carries && 'md:hidden'" bare :balance="props.balance" :from="props.month" :to="props.month" @pick="emit('pick', $event)" />
    <Table class="text-sm tabular-nums md:min-w-[720px]" container-class="max-md:hidden">
      <TableHeader>
        <TableRow class="text-xs">
          <TableHead class="text-muted-foreground md:w-[150px]">{{ props.balance.currency.toUpperCase() }}</TableHead>
          <TableHead v-for="head in heads" :key="head.month" :class="cn('text-right text-muted-foreground', head.picked && 'text-foreground')">
            {{ head.label }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="line in table" :key="line.key">
          <TableCell>{{ line.label }}</TableCell>
          <!-- The column head names the currency, so the mark in each cell is hidden (the sum stays hoverable). -->
          <TableCell v-for="value in line.cells" :key="value.month" :class="cn('text-right', line.class, value.class)">
            <Money bare :amount="value.amount" :currency="props.balance.currency" :decimals="0" :signed="line.signed" />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    <ul class="grid md:hidden">
      <li v-for="row in latestFirst" :key="row.month" class="flex min-h-11 items-center gap-2 border-b py-1.5 last:border-0">
        <Button variant="ghost" class="h-auto min-w-0 flex-1 flex-col items-start gap-0 px-0 font-normal" @click="emit('pick', row.month)">
          <span class="font-medium capitalize">{{ formatMonthKey(row.month, { month: 'long' }) }}</span>
          <span v-if="carries" class="text-xs text-muted-foreground"><i18n-t keypath="balance.totalNow">
              <template #total><Money :amount="row.total" :currency="props.balance.currency" :decimals="0" /></template>
            </i18n-t>
          </span>
        </Button>
        <b :class="['whitespace-nowrap', tone(row.left)]">
          <Money :amount="row.left" :currency="props.balance.currency" :decimals="0" signed />
        </b>
      </li>
    </ul>
  </section>
</template>
