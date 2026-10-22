<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatMoney, formatMonthKey } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/**
 * Twelve months as bars (Money 1 · 1): what came in (wide, grey), what went
 * (narrow, primary), the average month dashed across, and under each month
 * what it left. A month is a button: it opens that month. Twelve 40px targets
 * do not fit a phone, so there the bars only show (the period picker picks).
 */
/** `bare`: inside a card that has its own title. */
const props = defineProps<{ balance: MonthBalance; from: string; to: string; bare?: boolean }>()
const emit = defineEmits<{ pick: [month: string] }>()

const { t } = useI18n()
const phone = useIsPhone()
const top = computed(() => Math.max(1, ...props.balance.rows.flatMap((row) => [Number(row.income), Number(row.spent)])))
const height = (value: string) => `${Math.max(0, (Number(value) / top.value) * 100)}%`
const money = (value: string, signed = false) =>
  formatMoney(value, { decimals: 0, compact: Math.abs(Number(value)) >= 100_000, signed })
const left = (row: MonthBalanceRow) => money(row.left, true)
const span = computed(() => {
  const rows = props.balance.rows
  if (!rows.length) return ''
  const long = (key: string) => formatMonthKey(key, { month: 'long', year: 'numeric' }).replace(/\s*г\.$/, '')
  return `${long(rows[0].month)} — ${long(rows[rows.length - 1].month)}`
})
const average = computed(() => formatMoney(props.balance.average_spent, { decimals: 0, currency: props.balance.currency }))
const inPeriod = (key: string) => key >= props.from && key <= props.to
</script>

<template>
  <section :class="props.bare ? 'grid gap-1.5' : 'grid gap-1.5 rounded-xl border bg-card p-4 shadow-sm'" data-tour="spending-months">
    <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
      <template v-if="!props.bare">
        <b>{{ t('money.months.title') }}</b>
        <span class="text-muted-foreground max-md:hidden">{{ span }}</span>
      </template>
      <span class="flex-1" />
      <span class="flex items-center gap-1 text-muted-foreground max-md:hidden">
        <span class="inline-block size-2.5 rounded-[2px] bg-border" />{{ t('money.months.income') }}
      </span>
      <span class="flex items-center gap-1 text-muted-foreground max-md:hidden">
        <span class="inline-block size-2.5 rounded-[2px] bg-primary" />{{ t('money.months.spent') }}
      </span>
      <span class="text-muted-foreground">- - {{ t('money.months.average', { average }) }}</span>
      <span class="text-muted-foreground max-lg:hidden">· {{ t('money.months.leftHint') }}</span>
    </div>
    <div class="relative grid h-[170px] grid-cols-12 gap-1 md:gap-2">
      <div
        class="pointer-events-none absolute inset-x-0 border-t border-dashed border-muted-foreground/60"
        :style="{ bottom: `calc(44px + (100% - 44px) * ${Number(props.balance.average_spent) / top})` }"
        aria-hidden="true"
      />
      <component
        :is="phone ? 'div' : Button"
        v-for="row in props.balance.rows"
        :key="row.month"
        v-bind="phone ? {} : { variant: 'ghost' }"
        :class="cn('group flex h-auto min-w-0 flex-col items-center gap-0 rounded-md px-0 pt-2 pb-1 font-normal', inPeriod(row.month) && 'bg-accent/50')"
        :aria-label="`${formatMonthKey(row.month)}: ${money(row.spent)} / ${money(row.income)}`"
        v-on="phone ? {} : { click: () => emit('pick', row.month) }"
      >
        <span class="relative flex w-full flex-1 items-end justify-center">
          <span class="absolute bottom-0 w-[70%] max-w-[46px] rounded-[3px] bg-border" :style="{ height: height(row.income) }" />
          <span class="relative w-[34%] max-w-[22px] rounded-[2px] bg-primary" :style="{ height: height(row.spent) }" />
        </span>
        <span :class="cn('mt-1 text-xs whitespace-nowrap text-muted-foreground', inPeriod(row.month) && 'font-semibold text-foreground')">
          <span class="md:hidden">{{ formatMonthKey(row.month, { month: 'narrow' }) }}</span>
          <span class="max-md:hidden">{{ formatMonthKey(row.month, { month: 'short' }).replace('.', '') }}</span>
          <span v-if="row.current" class="max-lg:hidden">· {{ t('balance.running') }}</span>
        </span>
        <span :class="cn('text-xs font-semibold tabular-nums max-md:hidden', Number(row.left) < 0 ? 'text-warning' : 'text-success')">
          {{ left(row) }}
        </span>
      </component>
    </div>
  </section>
</template>
