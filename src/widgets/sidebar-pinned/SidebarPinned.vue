<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Filter, PinOff } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { usePins } from '@/shared/api/pins'
import { useStoreTree } from '@/shared/api/queries'
import { storeIcon } from '@/shared/lib/store-icons'
import { flattenStoreTree } from '@/shared/lib/store-tree'
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

import { partsText } from '@/shared/lib/money-parts'

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
const tree = useStoreTree()
const stores = computed(() => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store])))
/** A pinned wallet wears its own kind's icon, as in the wallets table; other entities their kind's. */
const iconOf = (view: PinnedView) => {
  if (view.pin.type !== 'entity') return Filter
  const store = view.pin.target.kind === 'store' ? stores.value.get(view.pin.target.id) : undefined
  return store ? storeIcon(store) : ENTITY_ICONS[view.pin.target.kind]
}
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
      tooltip: value ? `${name} · ${partsText([value])}` : name,
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
              <Money v-if="'amount' in row.value" v-bind="row.value" />
              <template v-else>{{ row.value.text }}</template>
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
