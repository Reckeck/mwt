<script setup lang="ts">
import { computed } from 'vue'

import { Money } from '@/entities/currency'
import { formatMonthKey } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

/**
 * A category's twelve months (Money 1 · 2): grey bars, the period's month in
 * primary with its sum over it, the usual month dashed across.
 */
const props = defineProps<{ title: string; cells: string[]; months: string[]; average: string | null; currency: string; to: string }>()

const top = computed(() => Math.max(1, Number(props.average ?? 0), ...props.cells.map(Number)))
const share = (value: string | number) => `${Math.max(0, (Number(value) / top.value) * 100)}%`
const compact = (value: string) => Math.abs(Number(value)) >= 100_000
const bars = computed(() =>
  props.months.map((month, at) => ({ month, value: props.cells[at] ?? '0', current: month === props.to })),
)
</script>

<template>
  <section class="grid gap-2 rounded-xl border bg-card p-3 shadow-sm">
    <p class="text-xs font-semibold">{{ props.title }}</p>
    <div class="relative grid h-24 grid-cols-12 gap-1.5 pt-4" role="img" :aria-label="props.title">
      <div
        v-if="props.average"
        class="pointer-events-none absolute inset-x-0 border-t border-dashed border-muted-foreground/60"
        :style="{ bottom: `calc(18px + (100% - 34px) * ${Number(props.average) / top})` }"
        aria-hidden="true"
      >
        <span class="absolute -top-4 left-0 text-2xs text-muted-foreground"><i18n-t keypath="money.category.average">
          <template #average><Money :amount="props.average" :currency="props.currency" :decimals="0" :compact="compact(props.average)" /></template>
        </i18n-t></span>
      </div>
      <div v-for="bar in bars" :key="bar.month" class="flex min-w-0 flex-col items-center gap-1">
        <span class="relative flex w-full flex-1 items-end justify-center">
          <span
            :class="cn('w-[60%] max-w-4 rounded-[2px]', bar.current ? 'bg-primary' : 'bg-muted-foreground/35')"
            :style="{ height: share(bar.value) }"
          />
          <span v-if="bar.current" class="absolute bottom-full mb-0.5 text-2xs font-semibold whitespace-nowrap tabular-nums" :style="{ bottom: share(bar.value) }">
            <Money :amount="bar.value" :currency="props.currency" :decimals="0" :compact="compact(bar.value)" />
          </span>
        </span>
        <span :class="cn('text-2xs text-muted-foreground', bar.current && 'font-semibold text-foreground')">
          {{ formatMonthKey(bar.month, { month: 'narrow' }) }}
        </span>
      </div>
    </div>
  </section>
</template>
