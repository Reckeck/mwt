<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronRight, Plus } from 'lucide-vue-next'

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
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { getStoreIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

const { t } = useI18n()
const route = useRoute()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

interface StoreNode {
  store: Store
  children: StoreNode[]
}

/** Live wallets first, archived last; alphabetical inside each group. */
function byStatusThenName(left: StoreNode, right: StoreNode): number {
  if (left.store.archived !== right.store.archived) return left.store.archived ? 1 : -1
  return left.store.name.localeCompare(right.store.name, undefined, { sensitivity: 'base' })
}

/** The flat store list as a tree over `parent_id`; an unknown parent makes a root. */
const roots = computed<StoreNode[]>(() => {
  const nodes = new Map<string, StoreNode>()
  for (const store of stores.data.value ?? []) nodes.set(store.id, { store, children: [] })
  const top: StoreNode[] = []
  for (const node of nodes.values()) {
    const parent = node.store.parent_id ? nodes.get(node.store.parent_id) : undefined
    if (parent) parent.children.push(node)
    else top.push(node)
  }
  const sortTree = (list: StoreNode[]) => {
    list.sort(byStatusThenName)
    for (const node of list) sortTree(node.children)
  }
  sortTree(top)
  return top
})

/** Value of every wallet including its sub-wallets, in the base currency. */
const subtreeValue = computed(() => {
  const own = new Map<string, number>()
  for (const row of summary.data.value?.by_store ?? []) own.set(row.store_id, Number(row.value_base))
  const totals = new Map<string, number>()
  const walk = (node: StoreNode): number => {
    const sum = node.children.reduce((acc, child) => acc + walk(child), own.get(node.store.id) ?? 0)
    totals.set(node.store.id, sum)
    return sum
  }
  for (const node of roots.value) walk(node)
  return totals
})

/** Every wallet below `node`, depth-first: the sub-menu is one level, so deeper ones are listed too. */
function descendants(node: StoreNode): StoreNode[] {
  return node.children.flatMap((child) => [child, ...descendants(child)])
}

function walletTotal(id: string): string {
  const value = subtreeValue.value.get(id)
  if (!value) return ''
  return formatMoney(value, { currency: summary.data.value?.base_currency ?? '', decimals: 0 })
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
</script>

<template>
  <SidebarGroup>
    <SidebarGroupLabel>{{ t('nav.wallets') }}</SidebarGroupLabel>
    <SidebarGroupAction as-child :title="t('nav.addWallet')">
      <RouterLink :to="{ name: 'stores' }">
        <Plus />
        <span class="sr-only">{{ t('nav.addWallet') }}</span>
      </RouterLink>
    </SidebarGroupAction>

    <SidebarGroupContent>
      <SidebarMenu>
        <template v-if="stores.isPending.value">
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

          <!-- A parent shows the chevron; its total is already in the children's rows. -->
          <SidebarMenuAction
            v-if="node.children.length"
            :class="cn('transition-transform', expanded.has(node.store.id) && 'rotate-90')"
            :title="node.store.name"
            @click="toggle(node.store.id)"
          >
            <ChevronRight />
          </SidebarMenuAction>
          <SidebarMenuBadge v-else-if="walletTotal(node.store.id)" class="tabular-nums">
            {{ walletTotal(node.store.id) }}
          </SidebarMenuBadge>

          <SidebarMenuSub v-if="node.children.length && expanded.has(node.store.id)">
            <SidebarMenuSubItem v-for="child in descendants(node)" :key="child.store.id">
              <SidebarMenuSubButton
                as-child
                :is-active="activeId === child.store.id"
                :class="cn(child.store.archived && 'opacity-50')"
              >
                <RouterLink :to="{ name: 'wallet', params: { id: child.store.id } }">
                  <component :is="getStoreIcon(child.store.kind)" :style="{ color: child.store.color }" />
                  <span>{{ child.store.name }}</span>
                </RouterLink>
              </SidebarMenuSubButton>
            </SidebarMenuSubItem>
          </SidebarMenuSub>
        </SidebarMenuItem>

        <SidebarMenuItem v-if="stores.isSuccess.value && !roots.length">
          <SidebarMenuButton as-child :tooltip="t('nav.addWallet')">
            <RouterLink :to="{ name: 'stores' }">
              <Plus />
              <span>{{ t('nav.addWallet') }}</span>
            </RouterLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroupContent>
  </SidebarGroup>
</template>
