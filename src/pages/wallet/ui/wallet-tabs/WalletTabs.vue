<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Receipt } from 'lucide-vue-next'

import { AssetRevalueButton } from '@/features/asset-revalue'
import { TransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton } from '@/features/transaction-edit'
import { UtilityBillDialog } from '@/features/utility-bill'
import { UtilityRemindersButton } from '@/features/utility-reminders'
import { AllocationPie } from '@/widgets/allocation-pie'
import { AssetDetailsSheet } from '@/widgets/asset-details'
import { StoreTransactions } from '@/widgets/store-transactions'
import { PropertySummaryCard, UtilityList } from '@/widgets/utility-list'
import { UtilityStats } from '@/widgets/utility-stats'
import { StorePositions } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Tabs } from '@/shared/ui/tabs'
import { txId } from '@/shared/lib/transactions'

/** Utilities (a home), positions and history — the wallet's working area. */
const props = defineProps<{
  store: Store
  storeId: string
  tabs: { value: string; label: string }[]
  positions: StorePosition[]
  openPositions: StorePosition[]
  base: string
  decimals: number
  includeChildren: boolean
  /** The oldest quote behind the prices, for «prices as of». */
  pricesAsOf?: string | null
}>()
const tab = defineModel<string>('tab', { required: true })
const emit = defineEmits<{ operation: [] }>()

const { t } = useI18n()
const billOpen = ref(false)
const detailsOpen = ref(false)
const picked = ref<StorePosition | null>(null)
/** A coin wallet or an exchange holds several assets: a ring shows their shares. */
const slices = computed(() =>
  ['crypto', 'exchange'].includes(props.store.kind) && props.positions.length > 1
    ? props.positions.map((position) => ({ label: position.asset.symbol.toUpperCase(), value: position.value_base }))
    : [],
)
function openPosition(position: StorePosition) {
  picked.value = position
  detailsOpen.value = true
}
</script>

<template>
  <!-- Always offer both tabs: a wallet with no positions can still have history. -->
  <Tabs v-model="tab" :tabs="props.tabs">
    <template #utilities>
      <div class="flex flex-col gap-4">
        <PropertySummaryCard :property-id="props.store.id" />
        <div class="flex flex-wrap justify-end gap-2">
          <UtilityRemindersButton :store="props.store" />
          <Button :disabled="props.store.archived" @click="billOpen = true">
            <Receipt class="size-4" />
            {{ t('utilityBill.open') }}
          </Button>
        </div>
        <UtilityStats :property-id="props.store.id" />
        <UtilityList :property-id="props.store.id" />
        <UtilityBillDialog v-model:open="billOpen" :property-id="props.store.id" />
      </div>
    </template>
    <template #positions>
      <EmptyState v-if="props.positions.length === 0" :title="t('wallet.empty')" :description="t('wallet.emptyHint')">
        <Button :disabled="props.store.archived" @click="emit('operation')">
          <Plus class="size-4" />
          {{ t('wallet.addOperation') }}
        </Button>
      </EmptyState>
      <StorePositions
        v-else
        data-tour="wallet-positions"
        clickable
        :positions="props.positions"
        :base-currency="props.base"
        :decimals="props.decimals"
        :prices-as-of="props.pricesAsOf"
        @open="openPosition"
      >
        <template v-if="props.store.kind === 'property'" #actions="{ position }">
          <AssetRevalueButton :asset="position.asset" />
        </template>
      </StorePositions>
      <AllocationPie
        v-if="slices.length"
        class="mt-4"
        :title="t('charts.byAsset')"
        :slices="slices"
        :currency="props.base"
      />
      <!-- Futures positions: shown, never counted in the totals. -->
      <StorePositions
        v-if="props.openPositions.length"
        class="mt-4"
        :title="t('wallet.openPositions')"
        :positions="props.openPositions"
        :base-currency="props.base"
        :decimals="props.decimals"
      />
      <AssetDetailsSheet
        v-model:open="detailsOpen"
        :position="picked"
        :store-id="props.storeId"
        :base-currency="props.base"
        :decimals="props.decimals"
      />
    </template>
    <template #history>
      <StoreTransactions :store-id="props.storeId" :include-children="props.includeChildren">
        <template #actions="{ tx }">
          <TransactionEditButton :tx="tx" :store-id="props.storeId" />
          <TransactionDelete :id="txId(tx)" />
        </template>
      </StoreTransactions>
    </template>
  </Tabs>
</template>
