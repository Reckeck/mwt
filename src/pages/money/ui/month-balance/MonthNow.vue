<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatDayMonth, formatMoney } from '@/shared/lib/format'
import { shiftMonth } from '@/shared/lib/period'
import { cn } from '@/shared/lib/utils'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

/**
 * The month's balance in one sentence and what it is made of (Money 3 · 15):
 * came, spent, brought from the month before, what is there now.
 */
const props = defineProps<{ balance: MonthBalance; row: MonthBalanceRow }>()

const { t } = useI18n()
const money = (value: string, signed = false) => formatMoney(value, { decimals: 0, signed, currency: props.balance.currency })
const monthName = (key: string, form: 'in' | 'since') => t(`period.${form}.${key.slice(5, 7)}`)

const lastDay = computed(() => props.balance.days[props.balance.days.length - 1] ?? null)
const headline = computed(() => {
  const total = money(props.row.total)
  if (Number(props.row.total) < 0) return t('balance.now.minus', { when: monthName(props.row.month, 'in'), total: money(props.row.total.replace('-', '')) })
  const end = lastDay.value?.forecast
  if (!props.row.current || !props.balance.per_day || !end) return t('balance.now.past', { when: monthName(props.row.month, 'in'), total })
  const short = Number(end) < 0
  return t(short ? 'balance.now.short' : 'balance.now.running', {
    total,
    perDay: money(props.balance.per_day),
    date: formatDayMonth(`${lastDay.value?.date}T12:00`),
    end: money(short ? end.replace('-', '') : end),
  })
})
const note = computed(() =>
  props.balance.carry_since
    ? t('balance.now.carried', { since: monthName(props.balance.carry_since, 'since') })
    : t('balance.now.alone'),
)

const lines = computed(() => [
  { key: 'income', label: t('balance.now.income', { when: monthName(props.row.month, 'in') }), text: money(props.row.income, true), class: 'text-success' },
  { key: 'spent', label: t('balance.spent'), text: money(`-${props.row.spent}`.replace('--', '')), class: 'text-destructive' },
  ...(props.balance.carry_since
    ? [{ key: 'carried', label: t('balance.now.from', { since: monthName(shiftMonth(props.row.month, -1), 'since') }), text: money(props.row.carried_in, true), class: '' }]
    : []),
  { key: 'total', label: t('balance.now.total'), text: money(props.row.total), class: '' },
])
</script>

<template>
  <section class="grid gap-1.5 md:col-span-2 rounded-xl border bg-card px-5 py-4 shadow-sm">
    <h2 class="text-xl font-bold text-balance first-letter:uppercase">{{ headline }}</h2>
    <p class="text-sm text-muted-foreground">{{ note }}</p>
  </section>
  <Table class="text-sm tabular-nums" container-class="rounded-xl border bg-card shadow-sm md:self-start">
    <TableHeader>
      <TableRow class="text-xs">
        <TableHead class="text-muted-foreground">{{ t('balance.now.parts') }}</TableHead>
        <TableHead class="text-right text-muted-foreground">{{ t('balance.now.sum') }}</TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      <TableRow v-for="line in lines" :key="line.key">
        <TableCell>{{ line.label }}</TableCell>
        <TableCell :class="cn('text-right font-semibold whitespace-nowrap', line.class)">{{ line.text }}</TableCell>
      </TableRow>
    </TableBody>
  </Table>
</template>
