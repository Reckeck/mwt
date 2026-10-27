<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { AssetRevalueButton } from '@/features/asset-revalue'
import { AssetDetailsSheet } from '@/widgets/asset-details'
import { StorePositions } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'

import { ExchangeAccounts } from '../exchange-accounts'
import { WalletAssets } from '../wallet-assets'
import { WalletExtras } from '../wallet-extras'
import { WalletIncoming } from '../wallet-incoming'

/**
 * The positions tab: an exchange by its sub-accounts and its spot by
 * currency (Wallets-1 · 7), any other wallet its assets as a table
 * (Wallets-1 · 6); a property keeps its revaluation. Losses and exchange
 * balances follow under them.
 */
const props = defineProps<{
  store: Store
  storeId: string
  positions: StorePosition[]
  openPositions: StorePosition[]
  base: string
  decimals: number
  pricesAsOf?: string | null
  /** An exchange's own wallet, with its sub-accounts and own spot. */
  exchange?: { children: StoreChildRow[]; ownBase: string; ownPositions: StorePosition[] } | null
  hasKey: boolean
}>()
const emit = defineEmits<{ operation: [] }>()
const { t } = useI18n()

const detailsOpen = ref(false)
const picked = ref<StorePosition | null>(null)
function openPosition(position: StorePosition) {
  picked.value = position
  detailsOpen.value = true
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <ExchangeAccounts
      v-if="props.exchange && props.exchange.children.length"
      :store="props.store"
      :children="props.exchange.children"
      :own-base="props.exchange.ownBase"
      :own-positions="props.exchange.ownPositions"
      :base="props.base"
      :decimals="props.decimals"
    />

    <EmptyState v-if="props.positions.length === 0" :title="t('wallet.empty')" :description="t('wallet.emptyHint')">
      <Button :disabled="props.store.archived" @click="emit('operation')">
        <Plus class="size-4" />
        {{ t('wallet.addOperation') }}
      </Button>
    </EmptyState>
    <StorePositions
      v-else-if="props.store.kind === 'property'"
      data-tour="wallet-positions"
      clickable
      :positions="props.positions"
      :base-currency="props.base"
      :decimals="props.decimals"
      :prices-as-of="props.pricesAsOf"
      @open="openPosition"
    >
      <template #actions="{ position }">
        <AssetRevalueButton :asset="position.asset" :store-id="props.store.id" />
      </template>
    </StorePositions>
    <WalletAssets
      v-else
      :positions="props.positions"
      :base="props.base"
      :decimals="props.decimals"
      :spot="Boolean(props.exchange)"
      :prices-as-of="props.pricesAsOf"
      @open="openPosition"
    />

    <WalletIncoming v-if="props.store.kind === 'crypto'" :store-id="props.storeId" />

    <!-- Futures positions: shown, never counted in the totals. -->
    <StorePositions
      v-if="props.openPositions.length"
      :title="t('wallet.openPositions')"
      :positions="props.openPositions"
      :base-currency="props.base"
      :decimals="props.decimals"
    />
    <WalletExtras :store="props.store" :has-key="props.hasKey" />

    <AssetDetailsSheet
      v-model:open="detailsOpen"
      :position="picked"
      :store-id="props.storeId"
      :base-currency="props.base"
      :decimals="props.decimals"
      :revaluable="props.store.kind === 'property'"
    />
  </div>
</template>
