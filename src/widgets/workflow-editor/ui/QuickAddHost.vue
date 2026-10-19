<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { type QuickAddChoice, QuickAddPanel } from '@/features/workflow-quick-add'
import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'

import type { useCanvasInsert } from '../model/use-canvas-insert'

/** The node search: in a sheet on a phone, at the cursor on a desktop. */
const props = defineProps<{
  state: ReturnType<typeof useCanvasInsert>['quick']['value']
  simple: boolean
  phone: boolean
}>()
const emit = defineEmits<{ pick: [choice: QuickAddChoice]; close: [] }>()

const { t } = useI18n()
</script>

<template>
  <Drawer v-if="props.phone" :open="Boolean(props.state)" @update:open="(value) => !value && emit('close')">
    <DrawerContent>
      <DrawerTitle class="sr-only">{{ t('workflow.mobile.add') }}</DrawerTitle>
      <QuickAddPanel
        v-if="props.state"
        docked
        :left="props.state.left"
        :top="props.state.top"
        :context="props.state.context"
        :simple="props.simple"
        @pick="(choice) => emit('pick', choice)"
        @close="emit('close')"
      />
    </DrawerContent>
  </Drawer>
  <QuickAddPanel
    v-else-if="props.state"
    :left="props.state.left"
    :top="props.state.top"
    :context="props.state.context"
    :simple="props.simple"
    @pick="(choice) => emit('pick', choice)"
    @close="emit('close')"
  />
</template>
