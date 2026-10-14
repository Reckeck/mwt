<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { widgetDef } from '@/widgets/dashboard-registry'
import type { UiAction } from '@/shared/ui/context-menu'
import { WidgetFrame } from '@/shared/ui/widget-frame'

/** One widget in its frame: title, link, menu and the component itself. */
const props = defineProps<{
  id: string
  pinned: boolean
  actions: UiAction[]
  widgetProps: Record<string, unknown>
}>()
const emit = defineEmits<{ pin: [pinned: boolean] }>()

const { t } = useI18n()
const def = computed(() => widgetDef(props.id))
</script>

<template>
  <WidgetFrame
    :title="t(def?.titleKey ?? '')"
    :pinned="props.pinned"
    :actions="props.actions"
    :to="def?.link"
    @pin="(value) => emit('pin', value)"
  >
    <component :is="def?.component" v-bind="props.widgetProps" />
  </WidgetFrame>
</template>
