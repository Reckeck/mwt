<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { GripVertical, PinOff } from 'lucide-vue-next'

import { ActionMenu } from '@/shared/ui/action-menu'
import { ContextMenu } from '@/shared/ui/context-menu'

import type { WidgetFrameProps } from './widget-frame.types'

const props = withDefaults(defineProps<WidgetFrameProps>(), { pinned: false, actions: () => [], to: undefined })
const emit = defineEmits<{ pin: [pinned: boolean] }>()
const { t } = useI18n()
</script>

<template>
  <ContextMenu :actions="props.actions">
  <div class="group/frame relative flex h-full min-h-0 flex-col">
    <!-- The header is the drag handle; hiding it pins the widget in place. -->
    <div
      v-if="!props.pinned"
      class="widget-drag-handle flex h-7 shrink-0 cursor-grab items-center gap-1 px-1 text-xs text-muted-foreground active:cursor-grabbing"
    >
      <GripVertical class="size-3.5 opacity-40 group-hover/frame:opacity-100" aria-hidden="true" />
      <RouterLink
        v-if="props.to"
        :to="props.to"
        class="min-w-0 flex-1 truncate font-medium hover:text-foreground hover:underline"
        @mousedown.stop
      >
        {{ props.title }}
      </RouterLink>
      <span v-else class="min-w-0 flex-1 truncate font-medium">{{ props.title }}</span>
      <span class="opacity-0 transition-opacity focus-within:opacity-100 group-hover/frame:opacity-100">
        <ActionMenu :actions="props.actions" :label="t('widgetFrame.menu', { title: props.title })" />
      </span>
    </div>
    <button
      v-else
      type="button"
      class="absolute right-1 top-1 z-10 flex size-6 items-center justify-center rounded-md bg-background/80 text-muted-foreground opacity-0 shadow-sm transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover/frame:opacity-100"
      :title="t('widgetFrame.unpin')"
      :aria-label="t('widgetFrame.unpin')"
      @click="emit('pin', false)"
    >
      <PinOff class="size-3.5" />
    </button>
    <!-- Every widget adapts to its cell through container queries. -->
    <!-- min-h, not h: a widget taller than its cell scrolls inside the frame instead of spilling out of its card.
         The frame already names the widget: a title the widget marks with `data-widget-title` is hidden,
         and a card header left with nothing else goes too (its body then gets the top padding back). -->
    <div
      class="@container min-h-0 flex-1 overflow-auto [&>*]:min-h-full [&_[data-widget-title]]:hidden [&_[data-card-header]:has(>[data-widget-title]:only-child)]:hidden [&_[data-card-header]:has(>[data-widget-title]:only-child)+*]:pt-3"
    >
      <slot />
    </div>
  </div>
  </ContextMenu>
</template>
