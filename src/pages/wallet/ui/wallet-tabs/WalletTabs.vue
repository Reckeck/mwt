<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, RefreshCw } from 'lucide-vue-next'

import { UtilityDialog } from '@/features/utility-edit'
import { CryptoOperations } from '@/widgets/crypto-operations'
import { ExchangeTerminal } from '@/widgets/exchange-terminal'
import { StoreTransactions } from '@/widgets/store-transactions'
import { Button } from '@/shared/ui/button'
import { Tabs, type TabDef } from '@/shared/ui/tabs'
import { formatAsOf } from '@/shared/lib/format'

import { isEngineDriven } from '../../lib/engine-wallet'
import { CryptoRelations } from '../crypto-relations'
import { ExchangeImportView } from '../exchange-import-view'
import { UtilitiesTab } from '../utilities-tab'
import { WalletPositions } from '../wallet-positions'

/**
 * The wallet's working area under its header: the tabs its kind needs
 * (Wallets-1 · 5–7). A coin wallet shows beside them how fresh its prices
 * are, with `Refresh`.
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
  /** An exchange's own wallet: its positions are its spot. */
  exchange?: boolean
  hasKey: boolean
  refreshing: boolean
  /** Driven by the crypto engine (spec E9). */
  engine?: boolean
  account?: CryptoAccountView | null
}>()
const tab = defineModel<string>('tab', { required: true })
const emit = defineEmits<{ operation: []; refresh: [] }>()

const { t } = useI18n()
/** `+ Item` beside the tabs (Wallets-2 · 10); on a phone the tab puts it under the list. */
const itemOpen = ref(false)
/** The followed account of an engine-driven wallet: the one condition, from the page's own helper. */
const engineAccountId = computed(() => (isEngineDriven(props.store) ? props.store.crypto_account_id : undefined))
</script>

<template>
  <Tabs v-model="tab" data-tour="wallet-tabs" :tabs="props.tabs" :more="props.more" :more-label="t('wallet.tab.more')">
    <template #end>
      <Button v-if="tab === 'utilities' && !props.store.archived" variant="outline" class="max-md:hidden" data-utility-new @click="itemOpen = true">
        <Plus class="size-4" />{{ t('utilityTab.newItem') }}
      </Button>
      <span
        v-if="props.store.kind === 'crypto' && tab === 'positions' && props.pricesAsOf"
        class="flex items-center gap-1 text-xs text-muted-foreground"
      >
        {{ t('wallet.pricesAt', { time: formatAsOf(props.pricesAsOf) }) }} ·
        <Button variant="link" size="sm" class="h-auto gap-1 p-0 text-xs touch-hit-xl" :disabled="props.refreshing" @click="emit('refresh')">
          <RefreshCw class="size-3" :class="props.refreshing && 'animate-spin'" />
          {{ t('wallet.refreshPrices') }}
        </Button>
      </span>
    </template>
    <template #utilities>
      <UtilitiesTab :store="props.store" @new-item="itemOpen = true" />
      <UtilityDialog v-model:open="itemOpen" :property-id="props.store.id" />
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
        :engine="props.engine"
        :account="props.account"
        @operation="emit('operation')"
      />
    </template>
    <template #history>
      <!-- A wallet the engine drives reads its operations from the network: its own list, read-only (spec E10). -->
      <CryptoOperations v-if="engineAccountId" :account-ids="[engineAccountId]" bare />
      <StoreTransactions v-else :store-id="props.storeId" :include-children="props.includeChildren" bare />
    </template>
    <template #terminal>
      <ExchangeTerminal :store-id="props.storeId" />
    </template>
    <template #import>
      <ExchangeImportView :store="props.store" :has-key="props.hasKey" />
    </template>
    <template #relations>
      <!-- Who the address dealt with, from its events (spec E15); only a wallet the engine drives has the tab. -->
      <CryptoRelations v-if="engineAccountId" :account-id="engineAccountId" />
    </template>
  </Tabs>
</template>
