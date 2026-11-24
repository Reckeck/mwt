<script setup lang="ts">
import { computed } from 'vue'

import { cn } from '@/shared/lib/utils'

import { PROGRESS_SIZE_CLASS, PROGRESS_TONE_CLASS } from './progress-bar.constants'
import type { ProgressBarProps } from './progress-bar.types'

/**
 * A share of something (a budget spent, a goal saved, a file sent). The fill is
 * full width and slides by `translateX`: a change animates the transform, not
 * `width`, which re-laid the page out on every frame (rule 17).
 */
const props = withDefaults(defineProps<ProgressBarProps>(), { tone: 'primary', size: 'md', label: undefined, marker: undefined })
const clamp = (percent: number) => Math.min(100, Math.max(0, Number.isFinite(percent) ? percent : 0))
const shown = computed(() => clamp(props.value))
const markerAt = computed(() => (props.marker === undefined ? null : clamp(props.marker)))
</script>

<template>
  <div
    :class="cn('relative overflow-hidden rounded-full bg-muted', PROGRESS_SIZE_CLASS[props.size])"
    v-bind="
      props.label
        ? { role: 'progressbar', 'aria-label': props.label, 'aria-valuenow': Math.round(shown), 'aria-valuemin': 0, 'aria-valuemax': 100 }
        : { 'aria-hidden': 'true' }
    "
  >
    <div
      :class="cn('h-full w-full rounded-full transition-transform duration-300 motion-reduce:transition-none', PROGRESS_TONE_CLASS[props.tone])"
      :style="{ transform: `translateX(${shown - 100}%)` }"
    />
    <!-- The limit's tick: a left offset on a 1px line, not a width (rule 17). -->
    <div v-if="markerAt !== null" class="absolute inset-y-0 w-0.5 -translate-x-1/2 bg-foreground" :style="{ left: `${markerAt}%` }" data-progress-marker />
  </div>
</template>
