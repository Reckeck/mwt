<script setup lang="ts">
import { ChevronRight } from 'lucide-vue-next'
import {
  ContextMenuItem,
  ContextMenuPortal,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
} from 'reka-ui'

import { cn } from '@/shared/lib/utils'

import {
  MENU_CONTENT_CLASS,
  MENU_ITEM_CLASS,
  MENU_ITEM_DESTRUCTIVE_CLASS,
  MENU_ITEM_ICON_CLASS,
  visibleActions,
} from './context-menu.constants'
import type { UiAction } from './context-menu.types'

const props = defineProps<{ actions: UiAction[] }>()
</script>

<template>
  <template v-for="(action, index) in visibleActions(props.actions)" :key="action.id">
    <ContextMenuSeparator v-if="action.separated && index > 0" class="-mx-1 my-1 h-px bg-border" />
    <ContextMenuSub v-if="action.children?.length">
      <ContextMenuSubTrigger :class="MENU_ITEM_CLASS">
        <component :is="action.icon" v-if="action.icon" :class="MENU_ITEM_ICON_CLASS" />
        <span class="flex-1">{{ action.label }}</span>
        <ChevronRight class="size-4 text-muted-foreground" />
      </ContextMenuSubTrigger>
      <ContextMenuPortal>
        <ContextMenuSubContent :class="MENU_CONTENT_CLASS" :side-offset="4">
          <ContextMenuEntries :actions="action.children" />
        </ContextMenuSubContent>
      </ContextMenuPortal>
    </ContextMenuSub>
    <ContextMenuItem
      v-else
      :disabled="Boolean(action.disabled)"
      :title="typeof action.disabled === 'string' ? action.disabled : undefined"
      :class="cn(MENU_ITEM_CLASS, action.destructive && MENU_ITEM_DESTRUCTIVE_CLASS)"
      @select="action.run?.()"
    >
      <component :is="action.icon" v-if="action.icon" :class="MENU_ITEM_ICON_CLASS" />
      <span class="flex flex-1 flex-col">
        {{ action.label }}
        <span v-if="typeof action.disabled === 'string'" class="text-xs text-muted-foreground">{{ action.disabled }}</span>
      </span>
      <kbd v-if="action.shortcut" class="ml-4 text-xs tracking-widest text-muted-foreground">{{ action.shortcut }}</kbd>
    </ContextMenuItem>
  </template>
</template>
