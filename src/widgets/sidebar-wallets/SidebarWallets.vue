<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { Money } from '@/entities/currency'
import { useWalletCatalog } from '@/entities/store'
import { ContextMenu } from '@/shared/ui/context-menu'
import {
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
} from '@/shared/ui/sidebar'
import { useSettingsStore } from '@/shared/store/settings'
import { withCryptoShown, withoutLoyalty } from '@/shared/lib/store-tree'

import { canAddOfKind, groupTotal } from './lib/totals'
import { useExpandedStores } from './model/use-expanded-stores'
import { useWalletMenu } from './model/use-wallet-menu'
import { SidebarWalletItem } from './ui/sidebar-wallet-item'
import { useStoreTree } from '@/shared/api/queries'

/** The wallets in the sidebar, grouped by kind, with their totals and menus. */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const settings = useSettingsStore()

// Totals come rolled up from the backend: the sidebar and the wallet page
// read the same numbers instead of each summing on its own.
const tree = useStoreTree()
const roots = computed(() => withCryptoShown(withoutLoyalty(tree.data.value ?? []), settings.cryptoEnabled))


const activeId = computed(() => (route.name === 'wallet' ? String(route.params.id ?? '') : ''))
const { isExpanded, toggle, collapseAll } = useExpandedStores(activeId, roots)

/* --- groups: one per wallet kind, in the catalog's order; empty kinds stay hidden --- */
const catalog = useWalletCatalog()
const groups = computed(() =>
  (catalog.data.value?.kinds ?? [])
    .map((info) => ({ info, nodes: roots.value.filter((node) => node.store.kind === info.kind) }))
    .filter((group) => group.nodes.length > 0),
)

/* --- a new wallet opens in place, then the app goes to it --- */
const createOpen = ref(false)
const createKind = ref<StoreKind | null>(null)
function openCreate(kind: StoreKind | null) {
  createKind.value = kind
  createOpen.value = true
}
function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}

const { subParent, subOpen, txStore, txOpen, walletActions, groupActions } = useWalletMenu(openCreate, collapseAll)
const addLabel = (kind: StoreKind) => t('stores.addOfKind', { kind: t(`stores.kind.${kind}`) })
</script>

<template>
  <template v-if="tree.isPending.value">
    <SidebarGroup class="py-1">
      <SidebarGroupLabel class="h-6 group-data-[collapsible=icon]:-mt-6">{{ t('nav.wallets') }}</SidebarGroupLabel>
      <SidebarMenu>
        <SidebarMenuItem v-for="index in 3" :key="index">
          <SidebarMenuSkeleton show-icon />
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroup>
  </template>

  <SidebarGroup v-for="group in groups" :key="group.info.kind" class="py-1" :data-tour="`sidebar-group-${group.info.kind}`">
    <ContextMenu :actions="groupActions(group.info, group.nodes.length)">
      <SidebarGroupLabel class="flex items-center gap-2 h-6 group-data-[collapsible=icon]:-mt-6">
        <span class="min-w-0 flex-1 truncate">{{ t(`stores.group.${group.info.kind}`) }}</span>
        <span
          v-if="Number(groupTotal(group.nodes, settings.baseCurrency).amount)"
          class="shrink-0 tabular-nums"
          :class="canAddOfKind(group.info, group.nodes.length) && 'mr-6'"
        >
          <Money v-bind="groupTotal(group.nodes, settings.baseCurrency)" />
        </span>
      </SidebarGroupLabel>
    </ContextMenu>
    <SidebarGroupAction
      class="top-1.5"
      v-if="canAddOfKind(group.info, group.nodes.length)"
      :title="addLabel(group.info.kind)"
      @click="openCreate(group.info.kind)"
    >
      <Plus />
      <span class="sr-only">{{ addLabel(group.info.kind) }}</span>
    </SidebarGroupAction>

    <SidebarGroupContent>
      <SidebarMenu>
        <SidebarWalletItem
          v-for="node in group.nodes"
          :key="node.store.id"
          :node="node"
          :active-id="activeId"
          :expanded="isExpanded(node.store.id)"
          :currency="settings.baseCurrency"
          :actions-of="walletActions"
          @toggle="toggle(node.store.id)"
        />
      </SidebarMenu>
    </SidebarGroupContent>
  </SidebarGroup>

  <SidebarGroup v-if="tree.isSuccess.value" class="py-1">
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarMenuButton :tooltip="t('nav.addWallet')" data-tour="sidebar-add-wallet" @click="openCreate(null)">
          <Plus />
          <span>{{ t('nav.addWallet') }}</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  </SidebarGroup>

  <StoreCreateDialog v-model:open="createOpen" :kind="createKind" @created="openWallet" />
  <StoreCreateDialog v-model:open="subOpen" :parent="subParent" @created="openWallet" />
  <TransactionCreateDialog v-if="txStore" v-model:open="txOpen" :store-id="txStore" />
</template>
