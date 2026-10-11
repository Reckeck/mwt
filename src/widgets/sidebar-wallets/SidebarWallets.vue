<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import {
  Archive,
  ArchiveRestore,
  ChevronRight,
  ChevronsDownUp,
  Copy,
  ExternalLink,
  FolderPlus,
  Plus,
  Repeat,
  Settings2,
} from 'lucide-vue-next'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { StoreCreateDialog } from '@/features/store-create'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { bindingLabel, useWalletCatalog } from '@/entities/store'
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
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { call, onEvent, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { formatMoney } from '@/shared/lib/format'
import { getStoreIcon, storeIcon } from '@/shared/lib/store-icons'
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

/* --- groups: one per wallet kind, in the catalog's order; empty kinds stay hidden --- */
const catalog = useWalletCatalog()

const groups = computed(() =>
  (catalog.data.value?.kinds ?? [])
    .map((info) => ({
      info,
      nodes: roots.value.filter((node) => node.store.kind === info.kind),
    }))
    .filter((group) => group.nodes.length > 0),
)

function groupTotal(nodes: StoreTreeNode[]): string {
  const sum = nodes.filter((node) => !node.store.archived).reduce((acc, node) => acc + Number(node.subtree_base), 0)
  if (!sum) return ''
  return formatMoney(sum, { currency: settings.baseCurrency, decimals: 0 })
}

/** A kind takes another wallet unless it is capped (cash) and the cap is reached. */
function canAddOfKind(info: WalletKindInfo, count: number): boolean {
  return info.user_creatable && (info.max_roots === null || count < info.max_roots)
}

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

/* --- right-click menus: everything one can do with a wallet, without opening it --- */
const queryClient = useQueryClient()
const subParent = ref<Store | null>(null)
const subOpen = computed({
  get: () => subParent.value !== null,
  set: (value: boolean) => {
    if (!value) subParent.value = null
  },
})
const txStore = ref('')
const txOpen = computed({
  get: () => txStore.value !== '',
  set: (value: boolean) => {
    if (!value) txStore.value = ''
  },
})

const archive = useMutation({
  mutationFn: ({ id, archived }: { id: string; archived: boolean }) => call('archive_store', { id, archived }),
  onSuccess: (_result, { archived }) => {
    toast.success(archived ? t('wallet.archived') : t('wallet.unarchived'))
    for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('wallet.archiveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

async function copyName(name: string) {
  try {
    await navigator.clipboard.writeText(name)
    toast.success(t('contacts.copied'))
  } catch {
    toast.error(t('contacts.copyFailed'))
  }
}

function walletActions(node: StoreTreeNode): UiAction[] {
  const store = node.store
  const isCash = store.kind === 'cash' && !store.parent_id
  const settingsTo = (tab: string) => () =>
    void router.push({ name: 'wallet-settings', params: { id: store.id }, query: { tab } })
  return [
    { id: 'open', label: t('actions.open'), icon: ExternalLink, run: () => void router.push({ name: 'wallet', params: { id: store.id } }) },
    { id: 'tx', label: t('wallet.addOperation'), icon: Plus, disabled: store.archived ? t('wallet.archivedHint') : false, run: () => (txStore.value = store.id) },
    { id: 'sub', label: t('stores.newSubAccountShort'), icon: FolderPlus, hidden: Boolean(store.parent_id) || !node.can_add_child, run: () => (subParent.value = store) },
    { id: 'settings', label: t('walletSettings.nav'), icon: Settings2, separated: true, run: settingsTo('general') },
    { id: 'automations', label: t('walletSettings.automations'), icon: Repeat, run: settingsTo('automations') },
    { id: 'copy', label: t('actions.copyName'), icon: Copy, run: () => void copyName(store.name) },
    {
      id: 'archive',
      label: store.archived ? t('stores.unarchive') : t('stores.archive'),
      icon: store.archived ? ArchiveRestore : Archive,
      destructive: !store.archived,
      separated: true,
      disabled: isCash ? t('errors.CASH_LOCKED') : false,
      run: () => archive.mutate({ id: store.id, archived: !store.archived }),
    },
  ]
}

function groupActions(info: WalletKindInfo, count: number): UiAction[] {
  return [
    {
      id: 'add',
      label: t('stores.addOfKind', { kind: t(`stores.kind.${info.kind}`) }),
      icon: Plus,
      hidden: !canAddOfKind(info, count),
      run: () => openCreate(info.kind),
    },
    { id: 'collapse', label: t('actions.collapseAll'), icon: ChevronsDownUp, run: collapseAll },
  ]
}

function collapseAll() {
  expanded.value = new Set()
  try {
    localStorage.setItem(EXPANDED_KEY, '[]')
  } catch {
    // Storage blocked: nothing to remember.
  }
}
</script>

<template>
  <template v-if="tree.isPending.value">
    <SidebarGroup>
      <SidebarGroupLabel>{{ t('nav.wallets') }}</SidebarGroupLabel>
      <SidebarMenu>
        <SidebarMenuItem v-for="index in 3" :key="index">
          <SidebarMenuSkeleton show-icon />
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroup>
  </template>

  <SidebarGroup v-for="group in groups" :key="group.info.kind" :data-tour="`sidebar-group-${group.info.kind}`">
    <ContextMenu :actions="groupActions(group.info, group.nodes.length)">
      <SidebarGroupLabel class="flex items-center gap-2">
        <span class="min-w-0 flex-1 truncate">{{ t(`stores.group.${group.info.kind}`) }}</span>
        <span v-if="groupTotal(group.nodes)" class="shrink-0 tabular-nums" :class="canAddOfKind(group.info, group.nodes.length) && 'mr-6'">
          {{ groupTotal(group.nodes) }}
        </span>
      </SidebarGroupLabel>
    </ContextMenu>
    <SidebarGroupAction
      v-if="canAddOfKind(group.info, group.nodes.length)"
      :title="t('stores.addOfKind', { kind: t(`stores.kind.${group.info.kind}`) })"
      @click="openCreate(group.info.kind)"
    >
      <Plus />
      <span class="sr-only">{{ t('stores.addOfKind', { kind: t(`stores.kind.${group.info.kind}`) }) }}</span>
    </SidebarGroupAction>

    <SidebarGroupContent>
      <SidebarMenu>
        <SidebarMenuItem v-for="node in group.nodes" :key="node.store.id">
          <ContextMenu :actions="walletActions(node)">
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
          </ContextMenu>

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
              <ContextMenu :actions="walletActions(child)">
              <SidebarMenuSubButton
                as-child
                :is-active="activeId === child.store.id"
                :class="cn(child.store.archived && 'opacity-50')"
              >
                <RouterLink :to="{ name: 'wallet', params: { id: child.store.id } }">
                  <component :is="storeIcon(child.store)" :style="{ color: child.store.color }" />
                  <span class="min-w-0 flex-1 truncate">{{ child.store.name }}</span>
                  <span
                    v-if="bindingLabel(child.store.binding) && !child.store.name.includes(bindingLabel(child.store.binding))"
                    class="shrink-0 text-[10px] uppercase text-muted-foreground"
                  >
                    {{ bindingLabel(child.store.binding) }}
                  </span>
                  <span v-if="nodeTotal(child)" class="shrink-0 text-xs tabular-nums text-muted-foreground">
                    {{ nodeTotal(child) }}
                  </span>
                </RouterLink>
              </SidebarMenuSubButton>
              </ContextMenu>
            </SidebarMenuSubItem>
          </SidebarMenuSub>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroupContent>
  </SidebarGroup>

  <SidebarGroup v-if="tree.isSuccess.value">
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
