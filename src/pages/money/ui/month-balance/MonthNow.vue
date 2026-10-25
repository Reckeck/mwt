<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { formatDayMonth } from '@/shared/lib/format'
import { shiftMonth } from '@/shared/lib/period'
import { cn } from '@/shared/lib/utils'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

/**
 * The month's balance in one sentence and what it is made of (Money 3 · 15):
 * came, spent, brought from the month before, what is there now.
 */
const props = defineProps<{ balance: MonthBalance; row: MonthBalanceRow }>()

const { t } = useI18n()
/** A phrase's argument: a word or a sum drawn as `<Money>`. */
type Arg = { text: string } | { amount: string }
const sum = (amount: string): Arg => ({ amount })
const word = (text: string): Arg => ({ text })
const unsigned = (value: string) => value.replace('-', '')
const monthName = (key: string, form: 'in' | 'since') => t(`period.${form}.${key.slice(5, 7)}`)

const lastDay = computed(() => props.balance.days[props.balance.days.length - 1] ?? null)
type Headline = { key: string; args: Record<string, Arg> }
const headline = computed((): Headline => {
  const total = sum(props.row.total)
  const when = word(monthName(props.row.month, 'in'))
  if (Number(props.row.total) < 0) return { key: 'balance.now.minus', args: { when, total: sum(unsigned(props.row.total)) } }
  const end = lastDay.value?.forecast
  if (!props.row.current || !props.balance.per_day || !end) return { key: 'balance.now.past', args: { when, total } }
  const short = Number(end) < 0
  return {
    key: short ? 'balance.now.short' : 'balance.now.running',
    args: {
      total,
      perDay: sum(props.balance.per_day),
      date: word(formatDayMonth(`${lastDay.value?.date}T12:00`)),
      end: sum(short ? unsigned(end) : end),
    },
  }
})
const note = computed(() =>
  props.balance.carry_since
    ? t('balance.now.carried', { since: monthName(props.balance.carry_since, 'since') })
    : t('balance.now.alone'),
)

const lines = computed(() => [
  { key: 'income', label: t('balance.now.income', { when: monthName(props.row.month, 'in') }), amount: props.row.income, signed: true, class: 'text-success' },
  { key: 'spent', label: t('balance.spent'), amount: `-${props.row.spent}`.replace('--', ''), signed: false, class: 'text-destructive' },
  ...(props.balance.carry_since
    ? [{ key: 'carried', label: t('balance.now.from', { since: monthName(shiftMonth(props.row.month, -1), 'since') }), amount: props.row.carried_in, signed: true, class: '' }]
    : []),
  { key: 'total', label: t('balance.now.total'), amount: props.row.total, signed: false, class: '' },
])
</script>

<template>
  <section class="grid gap-1.5 md:col-span-2 rounded-xl border bg-card px-5 py-4 shadow-sm">
    <h2 class="text-xl font-bold text-balance first-letter:uppercase"><i18n-t :keypath="headline.key">
        <template v-for="(arg, name) in headline.args" :key="name" #[name]>
          <Money v-if="'amount' in arg" :amount="arg.amount" :currency="props.balance.currency" :decimals="0" />
          <template v-else>{{ arg.text }}</template>
        </template>
      </i18n-t>
    </h2>
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
        <TableCell :class="cn('text-right font-semibold whitespace-nowrap', line.class)">
          <Money :amount="line.amount" :currency="props.balance.currency" :decimals="0" :signed="line.signed" />
        </TableCell>
      </TableRow>
    </TableBody>
  </Table>
</template>
