<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
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
const compact = (value: string) => Math.abs(Number(value)) >= 100_000
/** The bar's spoken name: a label read aloud, so the sums stay text. */
// money: plain — aria-label of a bar: a screen reader reads the text, a card cannot open there
const speak = (value: string) => formatMoney(value, { decimals: 0, compact: compact(value) })
const span = computed(() => {
  const rows = props.balance.rows
  if (!rows.length) return ''
  const long = (key: string) => formatMonthKey(key, { month: 'long', year: 'numeric' }).replace(/\s*г\.$/, '')
  return `${long(rows[0].month)} — ${long(rows[rows.length - 1].month)}`
})
const inPeriod = (key: string) => key >= props.from && key <= props.to
/** «ноя», «дек»: three letters fit a phone's column (Intl's «нояб.», «сент.» do not). */
const monthShort = (key: string) => formatMonthKey(key, { month: 'short' }).replace('.', '').slice(0, 3)
/**
 * On a phone one month shown names every other column back from the last one
 * (Money 1 · 1), and the picked one; a longer period names all twelve, smaller.
 */
const single = computed(() => props.from === props.to)
const named = (index: number, key: string) =>
  !single.value || inPeriod(key) || (props.balance.rows.length - 1 - index) % 2 === 0
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
      <span class="text-muted-foreground">
        - -
        <i18n-t keypath="money.months.average">
          <template #average><Money :amount="props.balance.average_spent" :currency="props.balance.currency" :decimals="0" /></template>
        </i18n-t>
      </span>
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
        v-for="(row, index) in props.balance.rows"
        :key="row.month"
        v-bind="phone ? {} : { variant: 'ghost', onClick: () => emit('pick', row.month) }"
        :class="cn('group flex h-auto min-w-0 flex-col items-center gap-0 rounded-md px-0 pt-2 pb-1 font-normal', inPeriod(row.month) && 'bg-accent/50')"
        :aria-label="`${formatMonthKey(row.month)}: ${speak(row.spent)} / ${speak(row.income)}`"
      >
        <span class="relative flex w-full flex-1 items-end justify-center">
          <span class="absolute bottom-0 w-[70%] max-w-[46px] rounded-[3px] bg-border" :style="{ height: height(row.income) }" />
          <span class="relative w-[34%] max-w-[22px] rounded-[2px] bg-primary" :style="{ height: height(row.spent) }" />
        </span>
        <span
          :class="
            cn(
              'mt-1 text-xs whitespace-nowrap text-muted-foreground',
              !single && 'max-md:text-[10px] max-md:tracking-tight',
              inPeriod(row.month) && 'font-semibold text-foreground',
            )
          "
        >
          <span class="md:hidden" :class="!named(index, row.month) && 'invisible'">{{ monthShort(row.month) }}</span>
          <span class="max-md:hidden">{{ formatMonthKey(row.month, { month: 'short' }).replace('.', '') }}</span>
          <span v-if="row.current" class="max-lg:hidden">· {{ t('balance.running') }}</span>
        </span>
        <span :class="cn('text-xs font-semibold tabular-nums max-md:hidden', Number(row.left) < 0 ? 'text-warning' : 'text-success')">
          <!-- Twelve narrow columns: the mark is in the legend, so it is hidden here (the sum stays hoverable). -->
          <Money bare :amount="row.left" :currency="props.balance.currency" :decimals="0" :compact="compact(row.left)" signed />
        </span>
      </component>
    </div>
  </section>
</template>
