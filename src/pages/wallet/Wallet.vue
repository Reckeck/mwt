<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { AutomationDialog } from '@/features/automation-edit'
import { useExchangeKeyStatus } from '@/features/exchange-key'
import { GoalDialog } from '@/features/goal-edit'
import { PlanCalculatorDialog, type CalculatorPlan } from '@/features/plan-calculator'
import { usePriceRefresh } from '@/features/price-refresh'
import { AdjustmentDialog } from '@/features/store-adjust'
import { StoreCreateDialog } from '@/features/store-create'
import { QuickAddDialog } from '@/widgets/quick-add'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { DepositSummary } from '@/widgets/deposit-summary'
import { PropertyValueCard } from '@/widgets/property-value'
import { VehicleCard } from '@/widgets/vehicle-card'
import { isHome, useStoreOverview } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { openQuickAdd } from '@/shared/lib/quick-add'
import { useSettingsStore } from '@/shared/store/settings'

import { walletFigure } from './lib/wallet-figure'
import { type WalletTabsKind, useWalletScope, useWalletTabs } from './model/use-wallet-view'
import { SubAccountsCard } from './ui/sub-accounts-card'
import { WalletHeader } from './ui/wallet-header'
import { WalletTabs } from './ui/wallet-tabs'

/** One wallet (Wallets-1 · 5–7): its sum and actions on top, then the tabs its kind needs. */
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const settingsStore = useSettingsStore()

// The shell keys this page by path, so the id is fixed for its lifetime.
const storeId = computed(() => String(route.params.id ?? ''))
const { includeChildren, scopeValue } = useWalletScope(storeId)
const { overview, base, store, isArchived, children, hasChildren, positions, ancestors, parentPath, canAddChild, ownShare } =
  useStoreOverview(storeId, includeChildren)
const decimals = computed(() => settingsStore.displayDecimals)

// A bonus programme is not a wallet: old links land on its tab (plan G1).
watch(
  () => store.value?.kind,
  (kind) => {
    if (kind === 'loyalty') void router.replace({ name: 'transactions-bonuses', query: { program: storeId.value } })
  },
  { immediate: true },
)

/* --- exchanges: the terminal and the import extend the page --- */
const isExchangeNode = computed(() => store.value?.kind === 'exchange' && !store.value.parent_id)
/** The terminal belongs to an exchange whose provider is chosen. */
const hasTerminal = computed(() => isExchangeNode.value && !!store.value?.exchange)
const { hasKey: bybitHasKey } = useExchangeKeyStatus(storeId, isExchangeNode)
const isPropertyRoot = computed(() => store.value?.kind === 'property' && !store.value.parent_id)
/** Utilities and meters belong to a home only (plan G2). */
const isHomeRoot = computed(() => isPropertyRoot.value && isHome(store.value?.property?.category))
const tabsKind = computed<WalletTabsKind>(() => {
  if (isExchangeNode.value) return 'exchange'
  if (isHomeRoot.value) return 'home'
  return store.value?.kind === 'crypto' || store.value?.kind === 'exchange' || store.value?.kind === 'property' ? 'coins' : 'money'
})
const { tab, tabs, more } = useWalletTabs(tabsKind, hasTerminal)

/**
 * The header's sum. An exchange always counts its sub-accounts in it and lists
 * its own spot by currency below, so it has no scope to choose.
 */
const rollUp = computed(() => isExchangeNode.value || (hasChildren.value && includeChildren.value))
const figure = computed(() => {
  const data = overview.data.value
  if (!data) return null
  return rollUp.value
    ? walletFigure(data.subtree_positions, data.subtree_base, base.value)
    : walletFigure(data.positions, data.own_base, base.value)
})
const exchange = computed(() => {
  const data = overview.data.value
  return isExchangeNode.value && data ? { children: data.children, ownBase: data.own_base, ownPositions: data.positions } : null
})
/** The header's buttons: an exchange opens «Сделка», any other wallet the short form on its kind. */
function act(kind: TxKind) {
  if (isExchangeNode.value) openDeal(kind)
  else openQuickAdd({ storeId: storeId.value, kind })
}

const priceRefresh = usePriceRefresh()

/* --- dialogs: one per action, never one inside another --- */
const transactionOpen = ref(false)
const adjustOpen = ref(false)
const calculatorOpen = ref(false)
/** Plan handed from the calculator to the transaction form. */
const plan = ref<CalculatorPlan | null>(null)
const subWalletOpen = ref(false)
const goalOpen = ref(false)
const automationOpen = ref(false)
/** «Сделка · <биржа>» opened on the kind of the header button. */
const dealOpen = ref(false)
const dealKind = ref<TxKind>('trade_spot')
const deal = computed(() => (store.value && isExchangeNode.value ? { store: store.value, kind: dealKind.value } : null))
function openDeal(kind: TxKind) {
  dealKind.value = kind
  dealOpen.value = true
}

function openTransaction() {
  plan.value = null
  transactionOpen.value = true
}
/** Calculator → transaction: close one dialog, open the other with the plan. */
function onApplyPlan(payload: CalculatorPlan) {
  calculatorOpen.value = false
  plan.value = { ...payload }
  transactionOpen.value = true
}
/**
 * Nothing recorded and nothing held: one first step instead of empty charts. A property
 * is never «empty»: its utilities live in its tabs, and a bill can be due before the
 * first operation (the dashboard showed it, the page hid the tab).
 */
const isEmpty = computed(
  () =>
    store.value?.kind !== 'property' &&
    !hasChildren.value &&
    overview.data.value?.tx_count === 0 &&
    !overview.data.value?.positions.length,
)

function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}
</script>

<template>
  <SectionState :loading="overview.isPending.value" :error="overview.error.value" skeleton="figure" @retry="overview.refetch()">
    <div v-if="store && overview.data.value && figure" class="flex flex-col gap-4">
      <WalletHeader
        :store="store"
        :ancestors="ancestors"
        :figure="figure"
        :can-add-child="canAddChild"
        :has-children="hasChildren && !isExchangeNode"
        :refreshing="priceRefresh.isPending.value"
        :exchange="isExchangeNode"
        @act="act"
        @operation="openTransaction"
        @adjust="adjustOpen = true"
        @revalue="adjustOpen = true"
        @sell="openTransaction"
        @calculator="calculatorOpen = true"
        @sub-account="subWalletOpen = true"
        @goal="goalOpen = true"
        @automation="automationOpen = true"
        @refresh="priceRefresh.mutate()"
      />

      <EmptyState v-if="isEmpty" :title="t('wallet.empty.title')" :description="t('wallet.empty.description')">
        <Button :disabled="isArchived" @click="openTransaction">{{ t('wallet.empty.action') }}</Button>
      </EmptyState>

      <template v-else>
        <!-- A wallet with sub-accounts chooses its scope; an exchange lists them as cards instead. -->
        <div v-if="hasChildren && !isExchangeNode" class="flex justify-end">
          <ToggleGroup v-model="scopeValue" type="single" variant="outline" size="sm" :aria-label="t('stores.scope')">
            <ToggleGroupItem value="all" class="px-2.5">{{ t('stores.withChildren') }}</ToggleGroupItem>
            <ToggleGroupItem value="own" class="px-2.5">{{ t('stores.onlyThis') }}</ToggleGroupItem>
          </ToggleGroup>
        </div>

        <SubAccountsCard
          v-if="hasChildren && !isExchangeNode"
          :store="store"
          :children="children"
          :own-base="overview.data.value.own_base"
          :own-share="ownShare"
          :currency="base"
          :can-add-child="canAddChild"
          @add="subWalletOpen = true"
        />

        <DepositSummary
          v-if="store.sub_type === 'savings' || store.sub_type === 'deposit' || store.sub_type === 'pds'"
          :store-id="store.id"
        />

        <PropertyValueCard v-if="isPropertyRoot && !isHomeRoot" :store="store" />
        <VehicleCard v-if="isPropertyRoot && store.property?.category === 'car'" :store="store" />

        <WalletTabs
          v-model:tab="tab"
          :store="store"
          :store-id="storeId"
          :tabs="tabs"
          :more="more"
          :positions="exchange ? exchange.ownPositions : positions"
          :open-positions="overview.data.value.open_positions"
          :base="base"
          :decimals="decimals"
          :include-children="rollUp"
          :prices-as-of="overview.data.value.prices_as_of"
          :exchange="exchange"
          :has-key="bybitHasKey"
          :refreshing="priceRefresh.isPending.value"
          @operation="openTransaction"
          @refresh="priceRefresh.mutate()"
        />
      </template>

      <QuickAddDialog v-if="deal" v-model:open="dealOpen" :deal="deal" />
      <TransactionCreateDialog v-model:open="transactionOpen" :store-id="storeId" :disabled="isArchived" :plan="plan" />
      <AdjustmentDialog v-model:open="adjustOpen" :store-id="storeId" />
      <PlanCalculatorDialog v-model:open="calculatorOpen" :base-currency="base" :exchange="isExchangeNode ? store?.name : null" @apply="onApplyPlan" />
      <GoalDialog v-model:open="goalOpen" :store-id="storeId" />
      <AutomationDialog v-model:open="automationOpen" :store-id="storeId" />
      <StoreCreateDialog v-model:open="subWalletOpen" :parent="store" :parent-path="parentPath" @created="openWallet" />
    </div>
    <div v-else class="flex flex-col items-center gap-3 py-16 text-center">
      <p class="text-sm text-muted-foreground">{{ t('wallet.notFound') }}</p>
      <Button variant="outline" size="sm" @click="router.push({ name: 'stores' })">{{ t('wallet.backToStores') }}</Button>
    </div>
  </SectionState>
</template>
