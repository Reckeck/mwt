<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown, ChevronRight } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'

import type { AdvancedFieldsProps } from './advanced-fields.types'

/**
 * Rarely used fields of a form, folded under one toggle so the form starts
 * with what nearly everyone fills. Opens by itself when `defaultOpen` turns on
 * (a hidden field got a value or a save pointed at one of them).
 */
const props = withDefaults(defineProps<AdvancedFieldsProps>(), {
  label: undefined,
  openLabel: undefined,
  tone: 'muted',
  defaultOpen: false,
})

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
      :class="
        cn(
          'flex w-fit items-center gap-1 rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          props.tone === 'primary' ? 'text-primary hover:underline' : 'text-muted-foreground hover:text-foreground',
          // The primary row folds back from under the fields it opened (OpForm-1 · 5).
          props.tone === 'primary' && open && 'order-last',
        )
      "
    >
      <ChevronDown v-if="props.tone === 'primary'" :class="cn('size-4 transition-transform', open && 'rotate-180')" />
      <ChevronRight v-else :class="cn('size-4 transition-transform', open && 'rotate-90')" />
      {{ (open && props.openLabel) || props.label || t('ui.advanced') }}
    </CollapsibleTrigger>
    <CollapsibleContent class="flex flex-col gap-4">
      <slot />
    </CollapsibleContent>
  </Collapsible>
</template>
