<script setup lang="ts">
import { SIGN_TONE_CLASS } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'

import type { StatCardProps } from './stat-card.types'

/** A labelled figure: the same size, weight and digits in every row of stats. */
const props = withDefaults(defineProps<StatCardProps>(), {
  value: undefined,
  hint: undefined,
  tone: 'neutral',
  loading: false,
})
</script>

<template>
  <Card size="sm" class="min-w-0">
    <div class="flex min-w-0 flex-col gap-1">
      <p class="truncate text-xs text-muted-foreground">
        <slot name="label">{{ props.label }}</slot>
      </p>
      <Skeleton v-if="props.loading" class="h-7 w-28" />
      <p v-else :class="cn('figure truncate', SIGN_TONE_CLASS[props.tone])">
        <slot>{{ props.value }}</slot>
      </p>
      <p v-if="props.hint || $slots.hint" class="truncate text-xs text-muted-foreground">
        <slot name="hint">{{ props.hint }}</slot>
      </p>
    </div>
  </Card>
</template>
