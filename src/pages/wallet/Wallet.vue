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
import { TransactionCreateDialog } from '@/features/transaction-create'
import { DepositSummary } from '@/widgets/deposit-summary'
import { ExchangeTerminal } from '@/widgets/exchange-terminal'
import { StoreChart, useStoreOverview } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'
import { Tabs } from '@/shared/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { useSettingsStore } from '@/shared/store/settings'

import { useWalletScope, useWalletTabs } from './model/use-wallet-view'
import { ExchangeImportView } from './ui/exchange-import-view'
import { WalletExtras } from './ui/wallet-extras'
import { SubAccountsCard } from './ui/sub-accounts-card'
import { WalletHeader } from './ui/wallet-header'
import { WalletStats } from './ui/wallet-stats'
import { WalletTabs } from './ui/wallet-tabs'

/** One wallet: header and actions, its numbers and sub-accounts, chart, positions and history. */
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const settingsStore = useSettingsStore()

// The shell keys this page by path, so the id is fixed for its lifetime.
const storeId = computed(() => String(route.params.id ?? ''))
const { includeChildren, scopeValue } = useWalletScope(storeId)
const { overview, pnl, base, store, isArchived, children, hasChildren, positions, ancestors, parentPath, canAddChild, ownShare } =
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
const { tab, tabs, view, views } = useWalletTabs(isPropertyRoot, hasTerminal)

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
    <div v-if="store && overview.data.value" class="flex flex-col gap-4">
      <WalletHeader
        :store="store"
        :ancestors="ancestors"
        :can-add-child="canAddChild"
        :has-children="hasChildren"
        :refreshing="priceRefresh.isPending.value"
        @operation="openTransaction"
        @adjust="adjustOpen = true"
        @calculator="calculatorOpen = true"
        @sub-account="subWalletOpen = true"
        @goal="goalOpen = true"
        @automation="automationOpen = true"
        @refresh="priceRefresh.mutate()"
      />

      <!-- An exchange is extended by its terminal and its import: one view at a time, tabs like the rest of the app. -->
      <div v-if="hasTerminal" class="w-fit max-w-full" :aria-label="t('wallet.view.title')" data-tour="exchange-views">
        <Tabs v-model="view" :tabs="views" />
      </div>

      <ExchangeTerminal v-if="view === 'terminal'" :store-id="storeId" />
      <ExchangeImportView v-else-if="view === 'import'" :store-id="storeId" :has-key="bybitHasKey" />

      <EmptyState v-else-if="isEmpty" :title="t('wallet.empty.title')" :description="t('wallet.empty.description')">
        <Button :disabled="isArchived" @click="openTransaction">{{ t('wallet.empty.action') }}</Button>
      </EmptyState>

      <template v-else>
        <!-- Only a wallet with sub-accounts has a scope to choose. -->
        <div v-if="hasChildren" class="flex justify-end">
          <ToggleGroup v-model="scopeValue" type="single" variant="outline" size="sm" :aria-label="t('stores.scope')">
            <ToggleGroupItem value="all" class="px-2.5">{{ t('stores.withChildren') }}</ToggleGroupItem>
            <ToggleGroupItem value="own" class="px-2.5">{{ t('stores.onlyThis') }}</ToggleGroupItem>
          </ToggleGroup>
        </div>

        <WalletStats
          :overview="overview.data.value"
          :pnl="pnl.data.value"
          :pnl-loading="pnl.isPending.value"
          :positions="positions"
          :rolled-up="hasChildren && includeChildren"
        />

        <SubAccountsCard
          v-if="hasChildren"
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

        <StoreChart :store-id="storeId" :base-currency="base" :include-children="includeChildren" />
        <WalletExtras :store="store" :has-key="bybitHasKey" />

        <WalletTabs
          v-model:tab="tab"
          :store="store"
          :store-id="storeId"
          :tabs="tabs"
          :positions="positions"
          :open-positions="overview.data.value.open_positions"
          :base="base"
          :decimals="decimals"
          :include-children="includeChildren"
          :prices-as-of="overview.data.value.prices_as_of"
          @operation="openTransaction"
        />
      </template>

      <TransactionCreateDialog v-model:open="transactionOpen" :store-id="storeId" :disabled="isArchived" :plan="plan" />
      <AdjustmentDialog v-model:open="adjustOpen" :store-id="storeId" />
      <PlanCalculatorDialog v-model:open="calculatorOpen" :base-currency="base" @apply="onApplyPlan" />
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
