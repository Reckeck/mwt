<script setup lang="ts">
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'

import type { ReportTermProps } from './report-term.types'

/** A column or figure name with its meaning on hover and keyboard focus. */
const props = defineProps<ReportTermProps>()
</script>

<template>
  <Tooltip v-if="props.hint">
    <TooltipTrigger
      class="cursor-help rounded-sm underline decoration-dotted decoration-muted-foreground/60 underline-offset-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      {{ props.label }}
    </TooltipTrigger>
    <TooltipContent side="top" class="max-w-64 text-pretty">{{ props.hint }}</TooltipContent>
  </Tooltip>
  <span v-else>{{ props.label }}</span>
</template>
