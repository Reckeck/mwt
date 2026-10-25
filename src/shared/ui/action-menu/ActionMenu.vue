<script setup lang="ts">
import { ref, useAttrs } from 'vue'
import { ChevronRight, MoreHorizontal } from 'lucide-vue-next'
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from 'reka-ui'

import {
  MENU_CONTENT_CLASS,
  MENU_ITEM_CLASS,
  MENU_ITEM_DESTRUCTIVE_CLASS,
  MENU_ITEM_ICON_CLASS,
  visibleActions,
} from '@/shared/ui/context-menu'
import { Button } from '@/shared/ui/button'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'

import type { ActionMenuProps } from './action-menu.types'
import ActionSheet from './ActionSheet.vue'

defineOptions({ inheritAttrs: false })
const props = defineProps<ActionMenuProps>()
/** The caller's `class` (a row's `relative z-10` over its stretched link) goes on the «⋯» button, the only element. */
const attrs = useAttrs()

const phone = useIsPhone()
const sheetOpen = ref(false)

const TRIGGER =
  'touch-hit-lg flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
</script>

<template>
  <!-- On a phone the same list comes up as a sheet from below. -->
  <template v-if="phone">
    <Button variant="ghost" size="icon" :class="cn(TRIGGER, attrs.class as string | undefined)" :aria-label="props.label" @click="sheetOpen = true">
      <MoreHorizontal class="size-4" />
    </Button>
    <ActionSheet v-model:open="sheetOpen" :actions="props.actions" v-bind="props.sheet" />
  </template>
  <!-- The «⋯» twin of the right-click menu: built from the same action list. -->
  <DropdownMenuRoot v-else :modal="false">
    <DropdownMenuTrigger :class="cn(TRIGGER, attrs.class as string | undefined)" :aria-label="props.label" :title="props.label">
      <MoreHorizontal class="size-4" />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent :class="MENU_CONTENT_CLASS" align="end" :side-offset="4">
        <template v-for="(action, index) in visibleActions(props.actions)" :key="action.id">
          <DropdownMenuSeparator v-if="action.separated && index > 0" class="-mx-1 my-1 h-px bg-border" />
          <DropdownMenuSub v-if="action.children?.length">
            <DropdownMenuSubTrigger :class="MENU_ITEM_CLASS">
              <component :is="action.icon" v-if="action.icon" :class="MENU_ITEM_ICON_CLASS" />
              <span class="flex-1">{{ action.label }}</span>
              <ChevronRight class="size-4 text-muted-foreground" />
            </DropdownMenuSubTrigger>
            <DropdownMenuPortal>
              <DropdownMenuSubContent :class="MENU_CONTENT_CLASS" :side-offset="4">
                <DropdownMenuItem
                  v-for="child in visibleActions(action.children)"
                  :key="child.id"
                  :disabled="Boolean(child.disabled)"
                  :class="cn(MENU_ITEM_CLASS, child.destructive && MENU_ITEM_DESTRUCTIVE_CLASS)"
                  @select="child.run?.()"
                >
                  <component :is="child.icon" v-if="child.icon" :class="MENU_ITEM_ICON_CLASS" />
                  {{ child.label }}
                </DropdownMenuItem>
              </DropdownMenuSubContent>
            </DropdownMenuPortal>
          </DropdownMenuSub>
          <DropdownMenuItem
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
          </DropdownMenuItem>
        </template>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
