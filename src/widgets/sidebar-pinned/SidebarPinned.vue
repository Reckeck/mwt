<script setup lang="ts">
import { computed } from 'vue'
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
} from '@/shared/ui/sidebar'
import { cn } from '@/shared/lib/utils'

import { pinnedLocation, pinnedValue } from './lib/pinned-item'

/**
 * «Favourites» (Shell · 7): pinned wallets, entities and saved filters with a
 * short value; folded to icons, the tooltip carries the name and the value.
 * Hidden while empty.
 */
const { t } = useI18n()
const route = useRoute()
const pins = usePins()

const nameOf = (view: PinnedView) =>
  view.title || (view.pin.type === 'entity' ? t(`entity.kind.${view.pin.target.kind}`) : '')
const iconOf = (view: PinnedView) => (view.pin.type === 'entity' ? ENTITY_ICONS[view.pin.target.kind] : Filter)
const keyOf = (view: PinnedView, index: number) =>
  view.pin.type === 'entity' ? `${view.pin.target.kind}:${view.pin.target.id}` : `filter:${view.pin.id || index}`
const actionsOf = (index: number): UiAction[] => [
  { id: 'unpin', label: t('pins.unpin'), icon: PinOff, run: () => pins.remove(index) },
]

/** What a row shows, worked out once per pin. */
const rows = computed(() =>
  (pins.query.data.value ?? []).map((view, index) => {
    const name = nameOf(view)
    const value = pinnedValue(view)
    return {
      index,
      key: keyOf(view, index),
      name,
      value,
      icon: iconOf(view),
      missing: view.missing,
      tooltip: value ? `${name} · ${value}` : name,
      to: pinnedLocation(view.pin, route.query),
      actions: actionsOf(index),
    }
  }),
)
</script>

<template>
  <template v-if="rows.length">
    <SidebarGroup class="py-1" data-tour="sidebar-pinned">
      <SidebarGroupLabel class="h-6 group-data-[collapsible=icon]:-mt-6">{{ t('sidebar.favorites') }}</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          <SidebarMenuItem v-for="row in rows" :key="row.key" class="group/pin">
            <ContextMenu :actions="row.actions">
              <SidebarMenuButton
                as-child
                :tooltip="row.tooltip"
                :class="cn(row.missing && 'opacity-50')"
              >
                <RouterLink :to="row.to">
                  <component :is="row.icon" />
                  <span>{{ row.name }}</span>
                </RouterLink>
              </SidebarMenuButton>
            </ContextMenu>
            <SidebarMenuBadge
              v-if="row.value"
              class="right-1 text-muted-foreground tabular-nums pointer-coarse:right-8 pointer-fine:group-hover/pin:opacity-0 pointer-fine:group-focus-within/pin:opacity-0"
            >
              {{ row.value }}
            </SidebarMenuBadge>
            <SidebarMenuAction
              class="pointer-fine:opacity-0 pointer-fine:group-hover/pin:opacity-100 pointer-fine:group-focus-within/pin:opacity-100"
              :title="t('pins.unpin')"
              :aria-label="t('pins.unpin')"
              @click="pins.remove(row.index)"
            >
              <PinOff />
            </SidebarMenuAction>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  </template>
</template>
