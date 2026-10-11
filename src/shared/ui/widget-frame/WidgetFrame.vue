<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { EyeOff, GripVertical, MoreHorizontal, PinOff, X } from 'lucide-vue-next'

import { ContextMenu } from '@/shared/ui/context-menu'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'

import type { WidgetFrameProps } from './widget-frame.types'

const props = withDefaults(defineProps<WidgetFrameProps>(), { pinned: false, actions: () => [] })
const emit = defineEmits<{ pin: [pinned: boolean]; remove: [] }>()
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
      <span class="min-w-0 flex-1 truncate font-medium">{{ props.title }}</span>
      <DropdownMenu align="end">
        <template #trigger>
          <button
            type="button"
            class="flex size-6 items-center justify-center rounded opacity-0 transition-opacity hover:bg-accent focus-visible:opacity-100 group-hover/frame:opacity-100"
            :aria-label="t('widgetFrame.menu', { title: props.title })"
          >
            <MoreHorizontal class="size-4" />
          </button>
        </template>
        <DropdownMenuItem value="pin" @select="emit('pin', true)">
          <EyeOff class="size-4 shrink-0" />
          {{ t('widgetFrame.pin') }}
        </DropdownMenuItem>
        <DropdownMenuItem value="remove" destructive @select="emit('remove')">
          <X class="size-4 shrink-0" />
          {{ t('widgetFrame.remove') }}
        </DropdownMenuItem>
      </DropdownMenu>
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
    <div class="@container min-h-0 flex-1 overflow-auto [&>*]:h-full">
      <slot />
    </div>
  </div>
  </ContextMenu>
</template>
