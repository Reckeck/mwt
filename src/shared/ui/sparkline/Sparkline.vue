<script setup lang="ts">
import { computed } from 'vue'

import type { SparklineProps } from './sparkline.types'

/**
 * A trend without axes: a plain SVG line in `currentColor`, so a tile can draw
 * dozens of them without a chart instance each. Width follows the box, height the class.
 */
const props = withDefaults(defineProps<SparklineProps>(), { area: true, label: undefined })

const VIEW_W = 100
const VIEW_H = 30

const points = computed(() => {
  const numbers = props.values.map(Number).filter(Number.isFinite)
  if (numbers.length < 2) return ''
  const low = Math.min(...numbers)
  const span = Math.max(...numbers) - low || 1
  const step = VIEW_W / (numbers.length - 1)
  return numbers.map((value, index) => `${(index * step).toFixed(2)},${(VIEW_H - ((value - low) / span) * (VIEW_H - 2) - 1).toFixed(2)}`).join(' ')
})
</script>

<template>
  <svg
    v-if="points"
    class="block h-8 w-full overflow-visible"
    :viewBox="`0 0 ${VIEW_W} ${VIEW_H}`"
    preserveAspectRatio="none"
    role="img"
    :aria-hidden="props.label ? undefined : 'true'"
  >
    <title>{{ props.label ?? '' }}</title>
    <polygon v-if="props.area" :points="`0,${VIEW_H} ${points} ${VIEW_W},${VIEW_H}`" fill="currentColor" opacity="0.12" />
    <polyline :points="points" fill="none" stroke="currentColor" stroke-width="1.5" vector-effect="non-scaling-stroke" stroke-linejoin="round" />
  </svg>
  <!-- Too few points still keep the line's height, so the tile does not shrink when data arrives. -->
  <div v-else class="h-8 w-full" aria-hidden="true" />
</template>
