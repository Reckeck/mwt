<script setup lang="ts">
import { useCoarsePointer } from '@/shared/lib/phone'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'

import type { TapHintProps } from './tap-hint.types'

/**
 * A word or an icon with its meaning: a tooltip on hover and keyboard focus with a
 * mouse, a popover on a tap on a touch screen (a tooltip never opens under a finger).
 */
const props = defineProps<TapHintProps>()
const coarse = useCoarsePointer()

const BASE_CLASS =
  'touch-hit-xl cursor-help rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
const LABEL_CLASS = `${BASE_CLASS} underline decoration-dotted decoration-muted-foreground/60 underline-offset-4`
</script>

<template>
  <template v-if="!props.hint">
    <template v-if="props.label">{{ props.label }}</template>
    <slot v-else />
  </template>
  <Popover v-else-if="coarse">
    <PopoverTrigger data-touch-inline :aria-label="props.ariaLabel" :class="props.label ? LABEL_CLASS : BASE_CLASS">
      <template v-if="props.label">{{ props.label }}</template>
      <slot v-else />
    </PopoverTrigger>
    <PopoverContent overlay="tap-hint" side="top" :collision-padding="12" class="w-auto max-w-64 p-2 text-xs text-pretty">{{ props.hint }}</PopoverContent>
  </Popover>
  <Tooltip v-else>
    <TooltipTrigger data-touch-inline :aria-label="props.ariaLabel" :class="props.label ? LABEL_CLASS : BASE_CLASS">
      <template v-if="props.label">{{ props.label }}</template>
      <slot v-else />
    </TooltipTrigger>
    <TooltipContent side="top" class="max-w-64 text-pretty">{{ props.hint }}</TooltipContent>
  </Tooltip>
</template>
