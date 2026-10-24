<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Receipt, RefreshCw } from 'lucide-vue-next'

import { UtilityBillDialog } from '@/features/utility-bill'
import { UtilityRemindersButton } from '@/features/utility-reminders'
import { ExchangeTerminal } from '@/widgets/exchange-terminal'
import { StoreTransactions } from '@/widgets/store-transactions'
import { PropertySummaryCard, UtilityList } from '@/widgets/utility-list'
import { UtilityStats } from '@/widgets/utility-stats'
import { Button } from '@/shared/ui/button'
import { Tabs, type TabDef } from '@/shared/ui/tabs'
import { formatAsOf } from '@/shared/lib/format'

import { ExchangeImportView } from '../exchange-import-view'
import { WalletPositions } from '../wallet-positions'
import { WalletRelations } from '../wallet-relations'

/**
 * The wallet's working area under its header: the tabs its kind needs
 * (Wallets-1 · 5–7). A coin wallet shows beside them how fresh its prices
 * are, with «обновить».
 */
const props = defineProps<{
  store: Store
  storeId: string
  tabs: TabDef[]
  more: TabDef[]
  positions: StorePosition[]
  openPositions: StorePosition[]
  base: string
  decimals: number
  includeChildren: boolean
  /** The oldest quote behind the prices, for «prices as of». */
  pricesAsOf?: string | null
  exchange?: { children: StoreChildRow[]; ownBase: string; ownPositions: StorePosition[] } | null
  hasKey: boolean
  refreshing: boolean
}>()
const tab = defineModel<string>('tab', { required: true })
const emit = defineEmits<{ operation: []; refresh: [] }>()

const { t } = useI18n()
const billOpen = ref(false)
</script>

<template>
  <Tabs v-model="tab" :tabs="props.tabs" :more="props.more" :more-label="t('wallet.tab.more')">
    <template #end>
      <span
        v-if="props.store.kind === 'crypto' && tab === 'positions' && props.pricesAsOf"
        class="flex items-center gap-1 text-xs text-muted-foreground"
      >
        {{ t('wallet.pricesAt', { time: formatAsOf(props.pricesAsOf) }) }} ·
        <Button variant="link" size="sm" class="h-auto gap-1 p-0 text-xs" :disabled="props.refreshing" @click="emit('refresh')">
          <RefreshCw class="size-3" :class="props.refreshing && 'animate-spin'" />
          {{ t('wallet.refreshPrices') }}
        </Button>
      </span>
    </template>
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
      <WalletPositions
        :store="props.store"
        :store-id="props.storeId"
        :positions="props.positions"
        :open-positions="props.openPositions"
        :base="props.base"
        :decimals="props.decimals"
        :prices-as-of="props.pricesAsOf"
        :exchange="props.exchange"
        :has-key="props.hasKey"
        @operation="emit('operation')"
      />
    </template>
    <template #history>
      <StoreTransactions :store-id="props.storeId" :include-children="props.includeChildren" bare />
    </template>
    <template #terminal>
      <ExchangeTerminal :store-id="props.storeId" />
    </template>
    <template #import>
      <ExchangeImportView :store-id="props.storeId" :has-key="props.hasKey" />
    </template>
    <template #relations>
      <WalletRelations :store-id="props.storeId" />
    </template>
  </Tabs>
</template>
