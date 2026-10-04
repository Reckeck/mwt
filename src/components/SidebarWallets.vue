<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus, Warehouse, ChevronRight } from 'lucide-vue-next'

import { formatMoney } from '@/lib/format'
import { call } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import { getStoreIcon } from '@/lib/store-icons'

defineProps<{ showLabels: boolean }>()

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

// Expanded state for tree nodes (persisted in localStorage)
const expandedIds = ref<Set<string>>(new Set())

onMounted(() => {
  try {
    const saved = localStorage.getItem('sidebar-expanded-stores')
    if (saved) {
      expandedIds.value = new Set(JSON.parse(saved))
    }
  } catch {
    /* ignore */
  }
})

const saveExpanded = () => {
  try {
    localStorage.setItem('sidebar-expanded-stores', JSON.stringify([...expandedIds.value]))
  } catch {
    /* ignore */
  }
}

const toggleExpand = (id: string) => {
  if (expandedIds.value.has(id)) {
    expandedIds.value.delete(id)
  } else {
    expandedIds.value.add(id)
  }
  saveExpanded()
}

const isExpanded = (id: string): boolean => {
  return expandedIds.value.has(id)
}

/** Build tree from flat list using parent_id */
const buildTree = (stores: Store[]): StoreTreeNode[] => {
  const map = new Map<string, StoreTreeNode>()
  const roots: StoreTreeNode[] = []

  // Create nodes
  for (const store of stores) {
    map.set(store.id, { ...store, children: [], level: 0 })
  }

  // Link children to parents
  for (const node of map.values()) {
    const parent = node.parent_id ? map.get(node.parent_id) : undefined
    if (parent) {
      parent.children.push(node)
    } else {
      roots.push(node)
    }
  }

  // Compute levels
  const setLevels = (nodes: StoreTreeNode[], level: number) => {
    for (const node of nodes) {
      node.level = level
      setLevels(node.children, level + 1)
    }
  }
  setLevels(roots, 0)

  // Sort: live first, then archived; alphabetical within each group
  const sortTree = (nodes: StoreTreeNode[]) => {
    nodes.sort((left, right) => {
      if (left.archived !== right.archived) return left.archived ? 1 : -1
      return left.name.localeCompare(right.name, undefined, { sensitivity: 'base' })
    })
    for (const node of nodes) sortTree(node.children)
  }
  sortTree(roots)

  return roots
}

interface StoreTreeNode extends Store {
  children: StoreTreeNode[]
  level: number
  subtreeValue?: number
}

/** Compute subtree values recursively from tree + byStore map */
const computeSubtreeValues = (nodes: StoreTreeNode[], byStoreMap: Map<string, number>): void => {
  for (const node of nodes) {
    const own = byStoreMap.get(node.id) ?? 0
    let sum = own
    computeSubtreeValues(node.children, byStoreMap)
    for (const child of node.children) {
      sum += (child as StoreTreeNode & { subtreeValue?: number }).subtreeValue ?? 0
    }
    ;(node as StoreTreeNode & { subtreeValue?: number }).subtreeValue = sum
  }
}

/** Flat list of stores in tree order (for rendering) */
const flattenTree = (nodes: StoreTreeNode[], expanded: Set<string>): StoreTreeNode[] => {
  const result: StoreTreeNode[] = []
  const walk = (nodes: StoreTreeNode[]) => {
    for (const node of nodes) {
      result.push(node)
      if (expanded.has(node.id) && node.children.length > 0) {
        walk(node.children)
      }
    }
  }
  walk(nodes)
  return result
}

const treeRoots = computed(() => {
  const list = stores.data.value ?? []
  return buildTree(list)
})

const byStoreMap = computed(() => {
  const map = new Map<string, number>()
  for (const row of summary.data.value?.by_store ?? []) {
    map.set(row.store_id, Number(row.value_base))
  }
  return map
})

const activeId = computed(() => (route.name === 'wallet' ? String(route.params.id ?? '') : ''))
const base = computed(() => summary.data.value?.base_currency ?? '')


/** Roll-up value including children */
const displayValue = (node: StoreTreeNode): string => {
  const val = (node as StoreTreeNode & { subtreeValue?: number }).subtreeValue
  if (val !== undefined) {
    return formatMoney(val, { currency: base.value, decimals: 0 })
  }
  // Fallback to direct value
  const direct = summary.data.value?.by_store.find((row) => row.store_id === node.id)
  return direct ? formatMoney(direct.value_base, { currency: base.value, decimals: 0 }) : ''
}

onMounted(() => {
  computeSubtreeValues(treeRoots.value, byStoreMap.value)
})
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto pb-2">
    <div
      class="flex items-center justify-between px-3 pb-1 pt-3"
      :class="!showLabels && 'justify-center px-1'"
    >
      <p
        v-if="showLabels"
        class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
      >
        {{ t('nav.wallets') }}
      </p>
      <RouterLink
        :to="{ name: 'stores' }"
        :title="t('nav.stores')"
        class="flex items-center justify-center rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
      >
        <Warehouse class="size-4" />
      </RouterLink>
    </div>

    <nav class="space-y-0.5 px-2">
      <RouterLink
        v-for="node in flattenTree(treeRoots, expandedIds)"
        :key="node.id"
        :to="{ name: 'wallet', params: { id: node.id } }"
        :title="showLabels ? undefined : node.name"
        class="flex items-center gap-2.5 rounded-md px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        :class="cn(activeId === node.id && 'bg-accent text-accent-foreground font-medium', !showLabels && 'justify-center px-0')"
        :style="{ paddingLeft: showLabels ? `${12 + node.level * 16}px` : '12px' }"
      >
        <span
          class="flex items-center gap-1 shrink-0"
          :class="node.archived && 'opacity-40'"
        >
          <ChevronRight
            v-if="node.children.length > 0"
            class="size-4 text-muted-foreground transition-transform duration-150"
            :class="{ 'rotate-90': isExpanded(node.id) }"
            @click.stop="toggleExpand(node.id)"
          />
          <ChevronRight
            v-else
            class="size-4 invisible"
          />
          <span
            class="flex size-6 shrink-0 items-center justify-center rounded-md ring-1 ring-background"
            :class="node.archived && 'opacity-40'"
            :style="{
              backgroundColor: `${node.color}1A`,
              color: node.color,
              borderColor: `${node.color}33`,
            }"
          >
            <component :is="getStoreIcon(node.kind)" class="size-3.5" />
          </span>
        </span>
        <span v-if="showLabels" class="min-w-0 flex-1 truncate">{{ node.name }}</span>
        <span
          v-if="showLabels && displayValue(node)"
          class="shrink-0 text-xs tabular-nums text-muted-foreground"
        >
          {{ displayValue(node) }}
        </span>
      </RouterLink>

      <RouterLink
        v-if="!stores.data.value?.length"
        :to="{ name: 'stores' }"
        class="flex items-center gap-2.5 rounded-md px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        :class="cn(!showLabels && 'justify-center px-0')"
      >
        <Plus class="size-4 shrink-0" />
        <span v-if="showLabels" class="truncate">{{ t('nav.addWallet') }}</span>
      </RouterLink>
    </nav>

    <RouterLink
      v-if="stores.data.value?.length"
      :to="{ name: 'stores' }"
      :title="showLabels ? undefined : t('nav.addWallet')"
      class="mx-2 mt-1 flex items-center gap-2.5 rounded-md border border-dashed border-border px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
      :class="cn(!showLabels && 'justify-center px-0')"
    >
      <Plus class="size-4 shrink-0" />
      <span v-if="showLabels" class="truncate">{{ t('nav.addWallet') }}</span>
    </RouterLink>
  </div>
</template>
