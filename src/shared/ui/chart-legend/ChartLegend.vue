<script setup lang="ts">
import { cn } from '@/shared/lib/utils'

import { LEGEND_SWATCH_CLASS } from './chart-legend.constants'
import type { ChartLegendProps } from './chart-legend.types'

/** The legend of a chart with several series: a swatch and a label each; a dashed swatch for a reference line. */
const props = defineProps<ChartLegendProps>()
</script>

<template>
  <ul v-if="props.items.length >= 2" class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
    <li v-for="item in props.items" :key="item.label" class="flex min-w-0 items-center gap-1.5">
      <span
        aria-hidden="true"
        :class="cn('inline-block shrink-0', item.dashed ? 'h-0 w-3 border-t-2 border-dashed bg-transparent' : 'size-2 rounded-full', LEGEND_SWATCH_CLASS[item.tone])"
      />
      <span class="truncate">{{ item.label }}</span>
    </li>
  </ul>
</template>
