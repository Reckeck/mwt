<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'

import type { AdvancedFieldsProps } from './advanced-fields.types'

/**
 * Rarely used fields of a form, folded under one toggle so the form starts
 * with what nearly everyone fills. Opens by itself when `defaultOpen` turns on
 * (a hidden field got a value or a save pointed at one of them).
 */
const props = withDefaults(defineProps<AdvancedFieldsProps>(), { label: undefined, defaultOpen: false })

const { t } = useI18n()
const open = ref(props.defaultOpen)
watch(
  () => props.defaultOpen,
  (wanted) => {
    if (wanted) open.value = true
  },
)
</script>

<template>
  <Collapsible v-model:open="open" class="flex flex-col gap-4">
    <CollapsibleTrigger
      class="flex w-fit items-center gap-1 rounded-md text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <ChevronRight :class="cn('size-4 transition-transform', open && 'rotate-90')" />
      {{ props.label ?? t('ui.advanced') }}
    </CollapsibleTrigger>
    <CollapsibleContent class="flex flex-col gap-4">
      <slot />
    </CollapsibleContent>
  </Collapsible>
</template>
