<script setup lang="ts">
import { cn } from '@/shared/lib/utils'

import { ICON_BADGE_SIZE_CLASS, ICON_BADGE_STROKE_WIDTH, ICON_BADGE_TONE_CLASS } from './icon-badge.constants'
import type { IconBadgeProps } from './icon-badge.types'

/** The circled glyph at the start of a row: sunken background, soft positive for income. */
const props = withDefaults(defineProps<IconBadgeProps>(), { size: 'md', tone: 'neutral' })
</script>

<template>
  <span
    :class="cn('inline-flex shrink-0 items-center justify-center rounded-full', ICON_BADGE_SIZE_CLASS[props.size].circle, ICON_BADGE_TONE_CLASS[props.tone])"
    aria-hidden="true"
  >
    <component :is="props.icon" :class="ICON_BADGE_SIZE_CLASS[props.size].glyph" :stroke-width="ICON_BADGE_STROKE_WIDTH" />
  </span>
</template>
