<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronRight, Plus } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import {
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from '@/shared/ui/sidebar'
import { call, onEvent } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { getStoreIcon } from '@/shared/lib/store-icons'
import { STORE_TREE_QUERY_KEY, storeAncestorIds } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const settings = useSettingsStore()

// Totals come rolled up from the backend: the sidebar and the wallet page
// read the same numbers instead of each summing on its own.
const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
})

const roots = computed(() => tree.data.value ?? [])

// New quotes change every total, wherever the refresh was started.
void onEvent<RefreshResult>('prices-updated', () => void tree.refetch())

/** A parent shows its whole subtree, a leaf its own value. */
function nodeTotal(node: StoreTreeNode): string {
  const value = node.children.length ? node.subtree_base : node.own_base
  if (!Number(value)) return ''
  return formatMoney(value, { currency: settings.baseCurrency, decimals: 0 })
}

const activeId = computed(() => (route.name === 'wallet' ? String(route.params.id ?? '') : ''))

const EXPANDED_KEY = 'sidebar-expanded-stores'

function readExpanded(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(EXPANDED_KEY) ?? '[]') as string[])
  } catch {
    return new Set()
  }
}

const expanded = ref(readExpanded())

// Opening a sub-account (from a link, a breadcrumb) unfolds its parents so it is visible.
watch(
  [activeId, roots],
  ([id, list]) => {
    if (!id || !list.length) return
    const parents = storeAncestorIds(list, id)
    if (parents.every((parent) => expanded.value.has(parent))) return
    expanded.value = new Set([...expanded.value, ...parents])
  },
  { immediate: true },
)

function toggle(id: string) {
  const next = new Set(expanded.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expanded.value = next
  try {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify([...next]))
  } catch {
    // Storage blocked: the tree just opens collapsed next time.
  }
}

/* --- a new wallet opens in place, then the app goes to it --- */
const createOpen = ref(false)

function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}
</script>

<template>
  <SidebarGroup>
    <SidebarGroupLabel>{{ t('nav.wallets') }}</SidebarGroupLabel>
    <SidebarGroupAction :title="t('nav.addWallet')" @click="createOpen = true">
      <Plus />
      <span class="sr-only">{{ t('nav.addWallet') }}</span>
    </SidebarGroupAction>

    <SidebarGroupContent>
      <SidebarMenu>
        <template v-if="tree.isPending.value">
          <SidebarMenuItem v-for="index in 3" :key="index">
            <SidebarMenuSkeleton show-icon />
          </SidebarMenuItem>
        </template>

        <SidebarMenuItem v-for="node in roots" :key="node.store.id">
          <SidebarMenuButton
            as-child
            :is-active="activeId === node.store.id"
            :tooltip="node.store.name"
            :class="cn(node.store.archived && 'opacity-50')"
          >
            <RouterLink :to="{ name: 'wallet', params: { id: node.store.id } }">
              <component :is="getStoreIcon(node.store.kind)" :style="{ color: node.store.color }" />
              <span>{{ node.store.name }}</span>
            </RouterLink>
          </SidebarMenuButton>

          <!-- A parent keeps its rolled-up total, shifted left of the chevron. -->
          <SidebarMenuBadge v-if="nodeTotal(node)" :class="cn('tabular-nums', node.children.length && 'right-8')">
            {{ nodeTotal(node) }}
          </SidebarMenuBadge>
          <SidebarMenuAction
            v-if="node.children.length"
            :class="cn('transition-transform', expanded.has(node.store.id) && 'rotate-90')"
            :title="node.store.name"
            @click="toggle(node.store.id)"
          >
            <ChevronRight />
          </SidebarMenuAction>

          <SidebarMenuSub v-if="node.children.length && expanded.has(node.store.id)">
            <SidebarMenuSubItem v-for="child in node.children" :key="child.store.id">
              <div class="flex items-center">
                <SidebarMenuSubButton
                  as-child
                  :is-active="activeId === child.store.id"
                  :class="cn('min-w-0 flex-1', child.store.archived && 'opacity-50')"
                >
                  <RouterLink :to="{ name: 'wallet', params: { id: child.store.id } }">
                    <component :is="getStoreIcon(child.store.kind)" :style="{ color: child.store.color }" />
                    <span class="min-w-0 flex-1 truncate">{{ child.store.name }}</span>
                    <span v-if="nodeTotal(child)" class="shrink-0 text-xs tabular-nums text-muted-foreground">
                      {{ nodeTotal(child) }}
                    </span>
                  </RouterLink>
                </SidebarMenuSubButton>
                <button
                  v-if="child.children.length"
                  type="button"
                  class="flex size-6 shrink-0 items-center justify-center rounded-md text-sidebar-foreground/70 hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none"
                  :title="child.store.name"
                  :aria-expanded="expanded.has(child.store.id)"
                  @click="toggle(child.store.id)"
                >
                  <ChevronRight
                    :class="cn('size-4 transition-transform', expanded.has(child.store.id) && 'rotate-90')"
                  />
                </button>
              </div>

              <!-- Third level: the deepest the tree goes. -->
              <SidebarMenuSub v-if="child.children.length && expanded.has(child.store.id)" class="mr-0 pr-0">
                <SidebarMenuSubItem v-for="leaf in child.children" :key="leaf.store.id">
                  <SidebarMenuSubButton
                    as-child
                    size="sm"
                    :is-active="activeId === leaf.store.id"
                    :class="cn(leaf.store.archived && 'opacity-50')"
                  >
                    <RouterLink :to="{ name: 'wallet', params: { id: leaf.store.id } }">
                      <component :is="getStoreIcon(leaf.store.kind)" :style="{ color: leaf.store.color }" />
                      <span class="min-w-0 flex-1 truncate">{{ leaf.store.name }}</span>
                      <span v-if="nodeTotal(leaf)" class="shrink-0 text-xs tabular-nums text-muted-foreground">
                        {{ nodeTotal(leaf) }}
                      </span>
                    </RouterLink>
                  </SidebarMenuSubButton>
                </SidebarMenuSubItem>
              </SidebarMenuSub>
            </SidebarMenuSubItem>
          </SidebarMenuSub>
        </SidebarMenuItem>

        <SidebarMenuItem v-if="tree.isSuccess.value && !roots.length">
          <SidebarMenuButton :tooltip="t('nav.addWallet')" @click="createOpen = true">
            <Plus />
            <span>{{ t('nav.addWallet') }}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroupContent>

    <StoreCreateDialog v-model:open="createOpen" @created="openWallet" />
  </SidebarGroup>
</template>
