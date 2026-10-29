<script setup lang="ts">
import { SIGN_TONE_CLASS } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'

import type { StatCardProps } from './stat-card.types'

/**
 * A labelled figure: the same size, weight and digits in every row of stats. In a card
 * narrower than 9rem (three in a row on a phone) the figure steps down a size and the
 * currency may wrap under the number instead of being cut to «RU…».
 */
const props = withDefaults(defineProps<StatCardProps>(), {
  value: undefined,
  hint: undefined,
  tone: 'neutral',
  loading: false,
})

const FIGURE_CLASS = 'figure truncate @max-[9rem]:text-base @max-[9rem]:whitespace-normal @max-[9rem]:[&_.money]:flex-wrap'
</script>

<template>
  <Card size="sm" class="@container min-w-0">
    <div class="flex min-w-0 flex-col gap-1">
      <p class="truncate text-xs text-muted-foreground">
        <slot name="label">{{ props.label }}</slot>
      </p>
      <Skeleton v-if="props.loading" class="h-7 w-28" />
      <p v-else :class="cn(FIGURE_CLASS, SIGN_TONE_CLASS[props.tone])">
        <slot>{{ props.value }}</slot>
      </p>
      <p v-if="props.hint || $slots.hint" class="truncate text-xs text-muted-foreground">
        <slot name="hint">{{ props.hint }}</slot>
      </p>
    </div>
  </Card>
</template>
