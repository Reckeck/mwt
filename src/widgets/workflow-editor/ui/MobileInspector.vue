<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'
import { Tabs } from '@/shared/ui/tabs'

import type { MobileInspectorTab } from '../workflow-editor.types'

/**
 * The phone's side panel: a sheet over the lower part of the canvas that
 * leaves the canvas usable (half height, or pulled up to full). Tabs: the
 * node or the workflow's settings, the run report, the lesson.
 */
const props = defineProps<{ hasRun: boolean; hasLesson: boolean; nodeSelected: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const tab = defineModel<MobileInspectorTab>('tab', { required: true })

const { t } = useI18n()
const SNAPS = [0.5, 1]
const snap = ref<number | string | null>(SNAPS[0])
/** At half height the lower half of the sheet is off screen: the scroll area ends at the screen's edge. */
const halfRoom = computed(() => (snap.value === SNAPS[0] ? { marginBottom: 'calc(50dvh - var(--safe-top) - 1.5rem)' } : undefined))

const tabs = computed(() => [
  { value: 'inspector', label: props.nodeSelected ? t('workflow.mobile.node') : t('workflow.mobile.settings') },
  ...(props.hasRun ? [{ value: 'run', label: t('workflow.mobile.run') }] : []),
  ...(props.hasLesson ? [{ value: 'lesson', label: t('workflow.mobile.lesson') }] : []),
])
</script>

<template>
  <Drawer v-model:open="open" :modal="false" :snap-points="SNAPS" v-model:snap-point="snap">
    <DrawerContent :overlay="false" class="h-[calc(100dvh-var(--safe-top)-1.5rem)] shadow-lg">
      <DrawerTitle class="sr-only">{{ tabs.find((entry) => entry.value === tab)?.label }}</DrawerTitle>
      <div class="px-3 pt-2">
        <Tabs :model-value="tab" :tabs="tabs" @update:model-value="(value) => (tab = value as MobileInspectorTab)" />
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-3" :style="halfRoom">
        <slot v-if="tab === 'inspector'" name="inspector" />
        <slot v-else-if="tab === 'run'" name="run" />
        <slot v-else name="lesson" />
      </div>
    </DrawerContent>
  </Drawer>
</template>
