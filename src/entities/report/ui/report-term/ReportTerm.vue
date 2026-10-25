<script setup lang="ts">
import { useCoarsePointer } from '@/shared/lib/phone'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'

import type { ReportTermProps } from './report-term.types'

/**
 * A column or figure name with its meaning: on hover and keyboard focus with a
 * mouse, on a tap on a touch screen (a tooltip never opens under a finger).
 */
const props = defineProps<ReportTermProps>()
const coarse = useCoarsePointer()

const TERM_CLASS =
  'touch-hit-xl cursor-help rounded-sm underline decoration-dotted decoration-muted-foreground/60 underline-offset-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
</script>

<template>
  <span v-if="!props.hint">{{ props.label }}</span>
  <Popover v-else-if="coarse">
    <PopoverTrigger data-touch-inline :class="TERM_CLASS">{{ props.label }}</PopoverTrigger>
    <PopoverContent overlay="report-term" side="top" class="w-auto max-w-64 p-2 text-xs text-pretty">{{ props.hint }}</PopoverContent>
  </Popover>
  <Tooltip v-else>
    <TooltipTrigger data-touch-inline :class="TERM_CLASS">{{ props.label }}</TooltipTrigger>
    <TooltipContent side="top" class="max-w-64 text-pretty">{{ props.hint }}</TooltipContent>
  </Tooltip>
</template>
