<script setup lang="ts">
import { cn } from '@/shared/lib/utils'

import { IconBadge } from '@/shared/ui/icon-badge'

import { LIST_ROW_SIZE_CLASS, LIST_ROW_TONE_CLASS } from './list-row.constants'
import type { ListRowProps } from './list-row.types'

/**
 * A row of a list of things (a wallet, a contact): icon circle, title with a
 * subtitle under it, and a value at the end in ink, positive or negative.
 * 56 px tall (64 px with `size="withControl"`); the owner makes it a link or a button by wrapping it.
 */
const props = withDefaults(defineProps<ListRowProps>(), { subtitle: undefined, icon: undefined, value: undefined, tone: 'ink', size: 'default' })
defineSlots<{ value?(): unknown }>()
</script>

<template>
  <div :class="cn('flex items-center gap-3 py-2', LIST_ROW_SIZE_CLASS[props.size])">
    <IconBadge v-if="props.icon" :icon="props.icon" size="md" data-list-row-icon />
    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium" data-list-row-title>{{ props.title }}</p>
      <p v-if="props.subtitle" class="truncate text-xs text-muted-foreground" data-list-row-subtitle>{{ props.subtitle }}</p>
    </div>
    <div
      v-if="props.value || $slots.value"
      :class="cn('shrink-0 whitespace-nowrap text-right text-sm font-medium tabular-nums', LIST_ROW_TONE_CLASS[props.tone])"
      data-list-row-value
    >
      <slot name="value">{{ props.value }}</slot>
    </div>
  </div>
</template>
