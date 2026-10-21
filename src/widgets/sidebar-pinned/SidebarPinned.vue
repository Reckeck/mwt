<script setup lang="ts">
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Filter, PinOff } from 'lucide-vue-next'

import { usePins } from '@/shared/api/pins'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { ENTITY_ICONS } from '@/shared/ui/entity-link'
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarSeparator,
} from '@/shared/ui/sidebar'
import { cn } from '@/shared/lib/utils'

import { pinnedLocation, pinnedValue } from './lib/pinned-item'

/** «Pinned»: entities and saved operation filters with a short value; hidden while empty. */
const { t } = useI18n()
const route = useRoute()
const pins = usePins()

const views = () => pins.query.data.value ?? []
const nameOf = (view: PinnedView) =>
  view.title || (view.pin.type === 'entity' ? t(`entity.kind.${view.pin.target.kind}`) : '')
const iconOf = (view: PinnedView) => (view.pin.type === 'entity' ? ENTITY_ICONS[view.pin.target.kind] : Filter)
const keyOf = (view: PinnedView, index: number) =>
  view.pin.type === 'entity' ? `${view.pin.target.kind}:${view.pin.target.id}` : `filter:${view.pin.id || index}`
const actionsOf = (index: number): UiAction[] => [
  { id: 'unpin', label: t('pins.unpin'), icon: PinOff, run: () => pins.remove(index) },
]
</script>

<template>
  <template v-if="views().length">
    <SidebarGroup class="py-1" data-tour="sidebar-pinned">
      <SidebarGroupLabel class="h-6 group-data-[collapsible=icon]:-mt-6">{{ t('pins.title') }}</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          <SidebarMenuItem v-for="(view, index) in views()" :key="keyOf(view, index)" class="group/pin">
            <ContextMenu :actions="actionsOf(index)">
              <SidebarMenuButton
                as-child
                :tooltip="nameOf(view)"
                :class="cn(view.missing && 'opacity-50')"
              >
                <RouterLink :to="pinnedLocation(view.pin, route.query)">
                  <component :is="iconOf(view)" />
                  <span>{{ nameOf(view) }}</span>
                </RouterLink>
              </SidebarMenuButton>
            </ContextMenu>
            <SidebarMenuBadge
              v-if="pinnedValue(view)"
              class="right-8 tabular-nums"
            >
              {{ pinnedValue(view) }}
            </SidebarMenuBadge>
            <SidebarMenuAction
              class="pointer-fine:opacity-0 pointer-fine:group-hover/pin:opacity-100 pointer-fine:group-focus-within/pin:opacity-100"
              :title="t('pins.unpin')"
              :aria-label="t('pins.unpin')"
              @click="pins.remove(index)"
            >
              <PinOff />
            </SidebarMenuAction>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
    <SidebarSeparator class="my-1" />
  </template>
</template>
