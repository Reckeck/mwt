<script setup lang="ts">
import { SIGN_TONE_CLASS } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import type { ChartTooltipProps } from './chart-tooltip.types'

/** The pinned readout of a chart: the point's title and one labelled figure per series. */
const props = defineProps<ChartTooltipProps>()
</script>

<template>
  <div class="min-w-32 rounded-sm border bg-popover px-3 py-2 text-xs text-popover-foreground">
    <p class="mb-1 text-muted-foreground">{{ props.title }}</p>
    <dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5">
      <template v-for="row in props.rows" :key="row.label">
        <dt class="truncate text-muted-foreground">{{ row.label }}</dt>
        <dd :class="cn('text-right tabular-nums', SIGN_TONE_CLASS[row.tone ?? 'neutral'])">{{ row.value }}</dd>
      </template>
    </dl>
  </div>
</template>
