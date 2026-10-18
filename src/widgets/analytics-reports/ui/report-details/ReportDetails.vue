<script setup lang="ts">
import { ChevronDown } from 'lucide-vue-next'

import { Card } from '@/shared/ui/card'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'

import type { ReportDetailsProps } from './report-details.types'

/** The figures behind a chart, folded under it until asked for. */
const props = withDefaults(defineProps<ReportDetailsProps>(), { defaultOpen: false })
</script>

<template>
  <Card>
    <Collapsible v-slot="{ open }" :default-open="props.defaultOpen">
      <CollapsibleTrigger
        class="flex w-full items-center gap-2 rounded-md text-left text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <ChevronDown class="size-4 shrink-0 transition-transform" :class="!open && '-rotate-90'" />
        {{ props.label }}
      </CollapsibleTrigger>
      <CollapsibleContent class="pt-3">
        <slot />
      </CollapsibleContent>
    </Collapsible>
  </Card>
</template>
