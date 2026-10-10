<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'
import {
  Archive,
  ArchiveRestore,
  Calculator,
  ChevronRight,
  FolderInput,
  FolderPlus,
  Key,
  MoreVertical,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
  Goal as GoalIcon,
  Repeat,
} from 'lucide-vue-next'

import { AssetRevalueButton } from '@/features/asset-revalue'
import { ExchangeKeyForm, useExchangeKeyStatus } from '@/features/exchange-key'
import { PendingImports } from '@/features/pending-imports'
import { PlanCalculatorDialog, type CalculatorPlan } from '@/features/plan-calculator'
import { StoreCreateDialog } from '@/features/store-create'
import { StoreEditDialog } from '@/features/store-edit'
import { StoreMoveDialog } from '@/features/store-move'
import { AutomationDialog } from '@/features/automation-edit'
import { GoalDialog } from '@/features/goal-edit'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton } from '@/features/transaction-edit'
import { StoreTransactions } from '@/widgets/store-transactions'
import { StoreChart, StorePositions } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { Tabs } from '@/shared/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call, parseError, type AppError } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { getStoreIcon } from '@/shared/lib/store-icons'
import { txId } from '@/shared/lib/transactions'
import { cn } from '@/shared/lib/utils'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const queryClient = useQueryClient()
const settingsStore = useSettingsStore()

// The shell keys this page by path, so the id is fixed for its lifetime.
const storeId = computed(() => String(route.params.id ?? ''))

/* --- scope: the wallet with its sub-accounts, or only itself; remembered per wallet --- */
const SCOPE_KEY = `wallet-only-own:${storeId.value}`

function readOnlyOwn(): boolean {
  try {
    return localStorage.getItem(SCOPE_KEY) === '1'
  } catch {
    return false
  }
}

const includeChildren = ref(!readOnlyOwn())

watch(includeChildren, (value) => {
  try {
    if (value) localStorage.removeItem(SCOPE_KEY)
    else localStorage.setItem(SCOPE_KEY, '1')
  } catch {
    // Storage blocked: the page just opens with sub-accounts next time.
  }
})

const scopeValue = computed({
  get: () => (includeChildren.value ? 'all' : 'own'),
  set: (value: string) => {
    if (value) includeChildren.value = value === 'all'
  },
})

const overview = useQuery({
  queryKey: computed(() => ['store', storeId.value, includeChildren.value]),
  queryFn: () =>
    call<StoreOverview>('get_store_overview', { storeId: storeId.value, includeChildren: includeChildren.value }),
  placeholderData: (previous) => previous,
})

const pnl = useQuery({
  queryKey: computed(() => ['store-pnl', storeId.value, includeChildren.value]),
  queryFn: () => call<RealizedPnl>('get_store_pnl', { storeId: storeId.value, includeChildren: includeChildren.value }),
})

const base = computed(() => overview.data.value?.base_currency ?? '')
const store = computed(() => overview.data.value?.store)
const isArchived = computed(() => store.value?.archived === true)
const decimals = computed(() => settingsStore.vault?.display_decimals ?? 8)
const children = computed(() => overview.data.value?.children ?? [])
const hasChildren = computed(() => children.value.length > 0)
/** With sub-accounts, positions are merged per asset across the whole subtree. */
const positions = computed(() => {
  const data = overview.data.value
  if (!data) return []
  return hasChildren.value && includeChildren.value ? data.subtree_positions : data.positions
})
const ancestors = computed(() => overview.data.value?.ancestors ?? [])
const parentPath = computed(() => [...ancestors.value.map((crumb) => crumb.name), store.value?.name ?? ''])
const canAddChild = computed(() => overview.data.value?.can_add_child === true)
const isSubAccount = computed(() => !!store.value?.parent_id)

const money = (value: string | undefined) => formatMoney(value ?? '0', { currency: base.value, decimals: 2 })
const share = (value: number) => `${Math.round(value * 100)}%`

/** Part of the subtree value that sits on the wallet itself. */
const ownShare = computed(() => {
  const total = Number(overview.data.value?.subtree_base ?? 0)
  return total ? Number(overview.data.value?.own_base ?? 0) / total : 0
})

/** Backend failures surface with their own code so the cause is visible. */
const errorText = computed(() => {
  const failure = overview.error.value as AppError | null
  if (!failure) return t('wallet.notFound')
  const known = t(`errors.${failure.code}`)
  return known === `errors.${failure.code}` ? failure.message : known
})

function pnlTone(value: string): string {
  const amount = Number(value)
  if (amount > 0) return 'text-emerald-600 dark:text-emerald-400'
  if (amount < 0) return 'text-destructive'
  return ''
}

/* --- dialogs + tabs: one dialog per action, never one inside another --- */
const transactionOpen = ref(false)
const calculatorOpen = ref(false)
/** Plan handed from the calculator to the transaction form. */
const plan = ref<CalculatorPlan | null>(null)
const editOpen = ref(false)
const subWalletOpen = ref(false)
const moveOpen = ref(false)
const goalOpen = ref(false)
const automationOpen = ref(false)
const tab = ref('positions')

function openTransaction() {
  plan.value = null
  transactionOpen.value = true
}

const tabs = computed(() => [
  { value: 'positions', label: t('pos.title') },
  { value: 'history', label: t('hist.title') },
])

const stats = computed(() => {
  const data = overview.data.value
  const rolledUp = hasChildren.value && includeChildren.value
  return [
    {
      key: 'total',
      label: rolledUp ? t('wallet.totalWithSub') : t('store.total'),
      value: money(rolledUp ? data?.subtree_base : data?.own_base),
      hint: rolledUp
        ? `${t('stores.ownFunds')}: ${money(data?.own_base)}`
        : `≈ ${formatMoney(data?.own_usd, { currency: 'usd', decimals: 2 })}`,
      tone: '',
    },
    {
      key: 'positions',
      label: t('pos.title'),
      value: String(positions.value.length),
      hint:
        positions.value
          .slice(0, 3)
          .map((position) => position.asset.symbol.toUpperCase())
          .join(' · ') || '—',
      tone: '',
    },
    {
      key: 'pnl',
      label: t('wallet.realizedPnl'),
      value: money(pnl.data.value?.total_base),
      hint: `${t('wallet.trades')}: ${pnl.data.value?.legs.length ?? 0}`,
      tone: pnlTone(pnl.data.value?.total_base ?? '0'),
    },
    {
      key: 'tx',
      label: t('wallet.transactions'),
      value: String(data?.tx_count ?? 0),
      hint: data?.last_tx_at ? `${t('wallet.lastTx')}: ${formatDate(data.last_tx_at)}` : '—',
      tone: '',
    },
  ]
})

/** Archiving takes the whole subtree along: say so before it happens. */
const archiveHint = computed(() =>
  !store.value?.archived && hasChildren.value ? t('stores.archiveCascade') : '',
)

/** Why delete is unavailable: the backend refuses a wallet with history or sub-accounts. */
const deleteBlocker = computed(() => {
  if (children.value.length) return t('stores.deleteHasChildren')
  if ((overview.data.value?.tx_count ?? 0) > 0) return t('stores.deleteHasTransactions')
  return ''
})

const refreshPrices = useMutation({
  mutationFn: () => call<RefreshResult>('refresh_all'),
  onSuccess: (result) => {
    toast.success(t('wallet.pricesRefreshed'), { description: `${result.prices} · ${result.fx}` })
    for (const queryKey of [['store'], ['store-history'], ['stores'], ['summary']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
  },
  onError: (err) => toast.error(t('wallet.pricesRefreshFailed'), { description: parseError(err).message }),
})

const toggleArchive = useMutation({
  mutationFn: (archived: boolean) => call('archive_store', { id: storeId.value, archived }),
  onSuccess: (_result, archived) => {
    toast.success(archived ? t('wallet.archived') : t('wallet.unarchived'))
    for (const queryKey of [['store'], ['stores'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('wallet.archiveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const destroy = useMutation({
  mutationFn: () => call('delete_store', { id: storeId.value }),
  onSuccess: () => {
    toast.success(t('wallet.deleted'))
    for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
    void router.push({ name: 'stores' })
  },
  onError: (err) => toast.error(t('wallet.deleteFailed'), { description: parseError(err).message }),
})

/* --- Bybit API key + import staging (only for exchange nodes) --- */
const isExchangeNode = computed(() => store.value?.kind === 'exchange')
const { hasKey: bybitHasKey } = useExchangeKeyStatus(storeId, isExchangeNode)

/** Calculator → transaction: close one dialog, open the other with the plan. */
function onApplyPlan(payload: CalculatorPlan) {
  calculatorOpen.value = false
  plan.value = { ...payload }
  transactionOpen.value = true
}

function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}
</script>

<template>
  <div v-if="overview.isPending.value" class="py-16 text-center text-sm text-muted-foreground">
    {{ t('store.loading') }}
  </div>

  <div v-else-if="overview.isError.value || !store" class="flex flex-col items-center gap-3 py-16 text-center">
    <p class="text-sm text-destructive">{{ errorText }}</p>
    <Button variant="outline" size="sm" @click="router.push({ name: 'stores' })">
      {{ t('wallet.backToStores') }}
    </Button>
  </div>

  <div v-else class="flex flex-col gap-6">
    <!-- Identity and every wallet action in one row. -->
    <div class="flex flex-col gap-3">
      <nav v-if="ancestors.length" class="flex flex-wrap items-center gap-1 text-xs text-muted-foreground">
        <template v-for="parent in ancestors" :key="parent.id">
          <RouterLink :to="{ name: 'wallet', params: { id: parent.id } }" class="hover:text-foreground hover:underline">
            {{ parent.name }}
          </RouterLink>
          <ChevronRight class="size-3" />
        </template>
        <span class="text-foreground">{{ store.name }}</span>
      </nav>

      <div class="flex flex-wrap items-center gap-x-4 gap-y-3">
        <span
          class="flex size-10 shrink-0 items-center justify-center rounded-lg border"
          :style="{ backgroundColor: `${store.color}1A`, color: store.color, borderColor: `${store.color}33` }"
        >
          <component :is="getStoreIcon(store.kind)" class="size-5" />
        </span>

        <!-- min width: on a phone the actions wrap below instead of squeezing the name. -->
        <div class="min-w-48 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="truncate text-lg font-semibold">{{ store.name }}</h2>
            <Badge variant="secondary">{{ t(`stores.kind.${store.kind}`) }}</Badge>
            <Badge v-if="store.archived" variant="outline">{{ t('stores.archived') }}</Badge>
            <Badge v-if="store.in_budget" variant="outline">{{ t('stores.inBudgetBadge') }}</Badge>
          </div>
          <p v-if="store.note" class="truncate text-sm text-muted-foreground">{{ store.note }}</p>
        </div>

        <div class="flex shrink-0 items-center gap-2">
          <Button size="sm" :disabled="isArchived" @click="openTransaction">
            <Plus class="size-4" />
            {{ t('wallet.addOperation') }}
          </Button>
          <Button v-if="canAddChild" variant="outline" size="sm" @click="subWalletOpen = true">
            <FolderPlus class="size-4" />
            {{ t('stores.newSubAccountShort') }}
          </Button>
          <Button
            variant="outline"
            size="icon"
            class="size-8"
            :title="t('calc.title')"
            :aria-label="t('calc.title')"
            @click="calculatorOpen = true"
          >
            <Calculator class="size-4" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            class="size-8"
            :title="t('store.refresh')"
            :aria-label="t('store.refresh')"
            :disabled="refreshPrices.isPending.value"
            @click="refreshPrices.mutate()"
          >
            <RefreshCw class="size-4" :class="cn(refreshPrices.isPending.value && 'animate-spin')" />
          </Button>

          <DropdownMenu align="end">
            <template #trigger>
              <Button variant="ghost" size="icon" class="size-8" :title="t('wallet.more')" :aria-label="t('wallet.more')">
                <MoreVertical class="size-4" />
              </Button>
            </template>
            <DropdownMenuItem value="edit" @select="editOpen = true">
              <Pencil class="size-4 shrink-0" />
              {{ t('stores.edit') }}
            </DropdownMenuItem>
            <DropdownMenuItem v-if="canAddChild" value="sub-account" @select="subWalletOpen = true">
              <FolderPlus class="size-4 shrink-0" />
              {{ t('stores.newSubAccountShort') }}
            </DropdownMenuItem>
            <DropdownMenuItem v-if="isSubAccount" value="move" @select="moveOpen = true">
              <FolderInput class="size-4 shrink-0" />
              {{ t('stores.move') }}
            </DropdownMenuItem>
            <DropdownMenuItem v-if="!store.archived" value="goal" @select="goalOpen = true">
              <GoalIcon class="size-4 shrink-0" />
              {{ t('goal.makeFromStore') }}
            </DropdownMenuItem>
            <DropdownMenuItem v-if="!store.archived" value="automation" @select="automationOpen = true">
              <Repeat class="size-4 shrink-0" />
              {{ t('automation.makeFromStore') }}
            </DropdownMenuItem>
            <DropdownMenuItem
              :value="store.archived ? 'unarchive' : 'archive'"
              @select="toggleArchive.mutate(!store.archived)"
            >
              <component :is="store.archived ? ArchiveRestore : Archive" class="size-4 shrink-0" />
              <span class="flex flex-col">
                {{ store.archived ? t('stores.unarchive') : t('stores.archive') }}
                <span v-if="archiveHint" class="text-xs text-muted-foreground">{{ archiveHint }}</span>
              </span>
            </DropdownMenuItem>
            <DropdownMenuItem
              value="delete"
              destructive
              :disabled="!!deleteBlocker"
              :title="deleteBlocker || undefined"
              @select="destroy.mutate()"
            >
              <Trash2 class="size-4 shrink-0" />
              <span class="flex flex-col">
                {{ t('stores.delete') }}
                <span v-if="deleteBlocker" class="text-xs text-muted-foreground">{{ deleteBlocker }}</span>
              </span>
            </DropdownMenuItem>
          </DropdownMenu>
        </div>
      </div>

      <p v-if="isArchived" class="rounded-md border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
        {{ t('wallet.archivedHint') }}
      </p>
    </div>

    <!-- Only a wallet with sub-accounts has a scope to choose. -->
    <div v-if="hasChildren" class="flex justify-end">
      <ToggleGroup v-model="scopeValue" type="single" variant="outline" size="sm" :aria-label="t('stores.scope')">
        <ToggleGroupItem value="all" class="px-2.5">{{ t('stores.withChildren') }}</ToggleGroupItem>
        <ToggleGroupItem value="own" class="px-2.5">{{ t('stores.onlyThis') }}</ToggleGroupItem>
      </ToggleGroup>
    </div>

    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <Card v-for="stat in stats" :key="stat.key">
        <p class="truncate text-xs text-muted-foreground">{{ stat.label }}</p>
        <p class="mt-1 truncate text-xl font-semibold tabular-nums" :class="stat.tone">{{ stat.value }}</p>
        <p class="mt-1 truncate text-xs text-muted-foreground">{{ stat.hint }}</p>
      </Card>
    </div>

    <Card v-if="hasChildren">
      <template #header>
        <div class="flex items-center justify-between gap-2">
          <p class="font-medium">{{ t('stores.subAccounts') }}</p>
          <Button v-if="canAddChild" variant="ghost" size="sm" @click="subWalletOpen = true">
            <FolderPlus class="size-4" />
            {{ t('stores.newSubAccountShort') }}
          </Button>
        </div>
      </template>
      <div class="flex flex-col divide-y divide-border">
        <!-- Money kept on the wallet itself, next to what its sub-accounts hold. -->
        <div class="flex items-center gap-3 py-2.5">
          <component :is="getStoreIcon(store.kind)" class="size-4 shrink-0" :style="{ color: store.color }" />
          <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ t('stores.ownFunds') }}</span>
          <span class="shrink-0 text-xs tabular-nums text-muted-foreground">{{ share(ownShare) }}</span>
          <span class="shrink-0 text-sm tabular-nums">{{ money(overview.data.value?.own_base) }}</span>
        </div>
        <RouterLink
          v-for="child in children"
          :key="child.store.id"
          :to="{ name: 'wallet', params: { id: child.store.id } }"
          class="-mx-2 flex items-center gap-3 rounded-md px-2 py-2.5 transition-colors hover:bg-accent"
          :class="child.store.archived && 'opacity-60'"
        >
          <component :is="getStoreIcon(child.store.kind)" class="size-4 shrink-0" :style="{ color: child.store.color }" />
          <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ child.store.name }}</span>
          <Badge variant="outline">{{ t(`stores.kind.${child.store.kind}`) }}</Badge>
          <span class="shrink-0 text-xs tabular-nums text-muted-foreground">{{ share(child.share) }}</span>
          <span class="shrink-0 text-sm tabular-nums">{{ money(child.subtree_base) }}</span>
        </RouterLink>
      </div>
    </Card>

    <Card v-if="isExchangeNode">
      <template #header>
        <div class="flex items-center gap-2">
          <Key class="size-4 text-muted-foreground" />
          <p class="font-medium">{{ t('wallet.bybit.title') }}</p>
        </div>
      </template>
      <div class="flex flex-col gap-4">
        <ExchangeKeyForm :store-id="storeId" />
        <PendingImports v-if="bybitHasKey" :store-id="storeId" />
      </div>
    </Card>

    <StoreChart :store-id="storeId" :base-currency="base" :include-children="includeChildren" />

    <!-- Always offer both tabs: a wallet with no positions can still have history. -->
    <Tabs v-model="tab" :tabs="tabs">
      <template #positions>
        <div
          v-if="positions.length === 0"
          class="flex flex-col items-center gap-2 rounded-xl border border-dashed border-border p-8 text-center"
        >
          <p class="text-sm font-medium">{{ t('wallet.empty') }}</p>
          <p class="text-sm text-muted-foreground">{{ t('wallet.emptyHint') }}</p>
          <Button class="mt-2" :disabled="isArchived" @click="openTransaction">
            <Plus class="size-4" />
            {{ t('wallet.addOperation') }}
          </Button>
        </div>
        <StorePositions v-else :positions="positions" :base-currency="base" :decimals="decimals">
          <template v-if="store.kind === 'asset'" #actions="{ position }">
            <AssetRevalueButton :asset="position.asset" />
          </template>
        </StorePositions>
        <!-- Futures positions: shown, never counted in the totals. -->
        <StorePositions
          v-if="overview.data.value?.open_positions.length"
          class="mt-4"
          :title="t('wallet.openPositions')"
          :positions="overview.data.value.open_positions"
          :base-currency="base"
          :decimals="decimals"
        />
      </template>
      <template #history>
        <StoreTransactions :store-id="storeId" :include-children="includeChildren">
          <template #actions="{ tx }">
            <TransactionEditButton :tx="tx" :store-id="storeId" />
            <TransactionDelete :id="txId(tx)" />
          </template>
        </StoreTransactions>
      </template>
    </Tabs>

    <TransactionCreateDialog v-model:open="transactionOpen" :store-id="storeId" :disabled="isArchived" :plan="plan" />
    <PlanCalculatorDialog v-model:open="calculatorOpen" :base-currency="base" @apply="onApplyPlan" />

    <StoreEditDialog v-model:open="editOpen" :store="store" />
    <StoreMoveDialog v-if="isSubAccount" v-model:open="moveOpen" :store="store" />
    <GoalDialog v-model:open="goalOpen" :store-id="storeId" />
    <AutomationDialog v-model:open="automationOpen" :store-id="storeId" />
    <StoreCreateDialog v-model:open="subWalletOpen" :parent="store" :parent-path="parentPath" @created="openWallet" />
  </div>
</template>
