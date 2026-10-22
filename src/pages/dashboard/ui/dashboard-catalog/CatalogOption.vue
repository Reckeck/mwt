<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import type { DashboardWidgetDef } from '@/widgets/dashboard-registry'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/** One widget in the catalog: a card that is pressed while the widget is on Home. */
const props = defineProps<{ def: DashboardWidgetDef; on: boolean }>()
const emit = defineEmits<{ toggle: [] }>()
const { t } = useI18n()
</script>

<template>
  <Button
    variant="outline"
    :aria-pressed="props.on"
    :class="
      cn(
        'grid h-auto min-h-16 content-start justify-stretch gap-0.5 p-3 text-left font-normal whitespace-normal',
        props.on && 'border-primary bg-primary/5',
      )
    "
    @click="emit('toggle')"
  >
    <span class="flex items-center gap-2 text-sm font-medium">
      <span class="flex-1">{{ t(props.def.titleKey) }}</span>
      <Check v-if="props.on" class="size-4 text-primary" aria-hidden="true" />
    </span>
    <span class="text-xs text-muted-foreground">{{ t(props.def.descriptionKey) }}</span>
  </Button>
</template>
