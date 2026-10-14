<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { formatPct, formatQuantity, signClass } from '@/shared/lib/format'

import type { ChartLegend } from '../../model/use-chart-core'

/** OHLC of the hovered bar (or the last one), on the same clock as the axis. */
const props = defineProps<{
  legend: ChartLegend
  title: string
  change: number
  fmt: (value: number) => string
}>()

const { t } = useI18n()
</script>

<template>
  <div
    class="pointer-events-none absolute left-2 top-2 z-10 flex max-w-[calc(100%-6rem)] flex-wrap items-center gap-x-3 gap-y-0.5 rounded-md bg-card/80 px-2 py-1 text-xs tabular-nums text-muted-foreground backdrop-blur-sm"
  >
    <span class="font-semibold text-foreground">{{ props.title }}</span>
    <span>{{ props.legend.time }}</span>
    <span>{{ t('trading.o') }} <span class="text-foreground">{{ props.fmt(props.legend.open) }}</span></span>
    <span>{{ t('trading.h') }} <span class="text-foreground">{{ props.fmt(props.legend.high) }}</span></span>
    <span>{{ t('trading.l') }} <span class="text-foreground">{{ props.fmt(props.legend.low) }}</span></span>
    <span>{{ t('trading.c') }} <span class="text-foreground">{{ props.fmt(props.legend.close) }}</span></span>
    <span :class="signClass(props.change)">{{ formatPct(props.change, { signed: true }) }}</span>
    <span>{{ t('trading.vol') }} <span class="text-foreground">{{ formatQuantity(props.legend.volume, 0) }}</span></span>
  </div>
</template>
