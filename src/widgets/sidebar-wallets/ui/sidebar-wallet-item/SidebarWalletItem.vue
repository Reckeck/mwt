<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { ChevronRight } from 'lucide-vue-next'

import { bindingBadge } from '@/entities/store'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import {
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from '@/shared/ui/sidebar'
import { storeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { nodeTotal } from '../../lib/totals'

/** A wallet in the sidebar: its link, total and menu, and its sub-accounts when unfolded. */
const props = defineProps<{
  node: StoreTreeNode
  activeId: string
  expanded: boolean
  currency: string
  actionsOf: (node: StoreTreeNode) => UiAction[]
}>()
const emit = defineEmits<{ toggle: [] }>()

const total = (node: StoreTreeNode) => nodeTotal(node, props.currency)
</script>

<template>
  <SidebarMenuItem>
    <ContextMenu :actions="props.actionsOf(props.node)">
      <SidebarMenuButton
        as-child
        :is-active="props.activeId === props.node.store.id"
        :tooltip="props.node.store.name"
        :class="cn(props.node.store.archived && 'opacity-50')"
      >
        <RouterLink :to="{ name: 'wallet', params: { id: props.node.store.id } }">
          <component :is="storeIcon(props.node.store)" class="store-tint" :style="{ '--store-color': props.node.store.color }" />
          <span>{{ props.node.store.name }}</span>
        </RouterLink>
      </SidebarMenuButton>
    </ContextMenu>

    <!-- A parent keeps its rolled-up total, shifted left of the chevron. -->
    <SidebarMenuBadge v-if="total(props.node)" :class="cn('tabular-nums', props.node.children.length && 'right-8')">
      {{ total(props.node) }}
    </SidebarMenuBadge>
    <SidebarMenuAction
      v-if="props.node.children.length"
      :class="cn('transition-transform', props.expanded && 'rotate-90')"
      :title="props.node.store.name"
      :aria-expanded="props.expanded"
      @click="emit('toggle')"
    >
      <ChevronRight />
    </SidebarMenuAction>

    <SidebarMenuSub v-if="props.node.children.length && props.expanded">
      <SidebarMenuSubItem v-for="child in props.node.children" :key="child.store.id">
        <ContextMenu :actions="props.actionsOf(child)">
          <SidebarMenuSubButton
            as-child
            :is-active="props.activeId === child.store.id"
            :class="cn(child.store.archived && 'opacity-50')"
          >
            <RouterLink :to="{ name: 'wallet', params: { id: child.store.id } }">
              <component :is="storeIcon(child.store)" class="store-tint" :style="{ '--store-color': child.store.color }" />
              <span class="min-w-0 flex-1 truncate">{{ child.store.name }}</span>
              <span v-if="bindingBadge(child.store)" class="shrink-0 text-2xs uppercase text-muted-foreground">
                {{ bindingBadge(child.store) }}
              </span>
              <span v-if="total(child)" class="shrink-0 text-xs tabular-nums text-muted-foreground">
                {{ total(child) }}
              </span>
            </RouterLink>
          </SidebarMenuSubButton>
        </ContextMenu>
      </SidebarMenuSubItem>
    </SidebarMenuSub>
  </SidebarMenuItem>
</template>
