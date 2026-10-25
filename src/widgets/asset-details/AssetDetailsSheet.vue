<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { PriceAlertList } from '@/widgets/price-alert-list'
import { TradingChart } from '@/widgets/trading-chart'
import { AssetRevalueButton } from '@/features/asset-revalue'
import { PriceAlertDialog, type PriceAlertPreset } from '@/features/price-alert-edit'
import { Money, PnlText } from '@/entities/currency'
import { priceOriginText } from '@/entities/store'
import { buttonVariants } from '@/shared/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { hereBackQuery } from '@/shared/lib/drill'
import { formatPct, formatQuantity } from '@/shared/lib/format'

import type { AssetDetailsSheetProps } from './asset-details.types'
import AssetElsewhere from './ui/AssetElsewhere.vue'

/** One position up close: its numbers, where the price is from, a chart, its alerts and links. */
const props = defineProps<AssetDetailsSheetProps>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const asset = computed(() => props.position?.asset ?? null)
const symbol = computed(() => asset.value?.symbol.toUpperCase() ?? '')
/** Binance quotes coins against USDT; a currency or a flat has no candles. */
const isCrypto = computed(() => !!asset.value?.chain && asset.value.chain !== 'fiat')
const where = computed(() =>
  asset.value?.chain && asset.value.chain !== 'fiat' ? CHAIN_LABELS[asset.value.chain] : (asset.value?.name ?? ''),
)
const pnlPct = computed(() => {
  const position = props.position
  const paid = Number(position?.avg_cost ?? 0) * Math.abs(Number(position?.quantity ?? 0))
  return position && paid ? (Number(position.unrealized_base) / paid) * 100 : null
})
const change = computed(() => props.position?.change_24h_pct ?? null)

const preset = computed<PriceAlertPreset | null>(() =>
  props.position ? { symbol: symbol.value, price: props.position.price_usd } : null,
)
const alertOpen = ref(false)
const chartPreset = ref<PriceAlertPreset | null>(null)
function alertAt(price: string, condition: 'above' | 'below') {
  chartPreset.value = { symbol: symbol.value, price, condition }
  alertOpen.value = true
}

const txLink = computed(() => ({ name: 'transactions', query: { store: props.storeId, q: symbol.value, ...hereBackQuery() } }))
const analyticsLink = computed(() => ({ name: 'money-investments', query: { store: props.storeId } }))
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent overlay="asset-details" side="right" class="w-full overflow-y-auto sm:max-w-2xl">
      <template v-if="props.position && asset">
        <SheetHeader>
          <SheetTitle>{{ symbol }}</SheetTitle>
          <SheetDescription>{{ where }}</SheetDescription>
        </SheetHeader>

        <div class="grid gap-5 px-4 pb-6">
          <dl class="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
            <div>
              <dt class="text-xs text-muted-foreground">{{ t('pos.value') }}</dt>
              <dd class="text-lg font-semibold">
                <Money :amount="props.position.value_base" :currency="props.baseCurrency" :decimals="2" />
              </dd>
            </div>
            <div>
              <dt class="text-xs text-muted-foreground">{{ t('pos.quantity') }}</dt>
              <dd class="tabular-nums">{{ formatQuantity(props.position.quantity, props.decimals) }}</dd>
            </div>
            <div>
              <dt class="text-xs text-muted-foreground">{{ t('assetDetails.priceNow') }}</dt>
              <dd>
                <Money :amount="props.position.price_base" :currency="props.baseCurrency" />
                <span
                  v-if="change !== null"
                  class="ml-2 text-xs tabular-nums"
                  :class="Number(change) < 0 ? 'text-destructive' : 'text-success'"
                >
                  {{ formatPct(change, { signed: true }) }} {{ t('pos.change24h') }}
                </span>
                <p class="text-xs text-muted-foreground">{{ priceOriginText(props.position.price_info) }}</p>
              </dd>
            </div>
            <div>
              <dt class="text-xs text-muted-foreground">{{ t('pos.avgCost') }}</dt>
              <dd>
                <Money v-if="props.position.avg_cost" :amount="props.position.avg_cost" :currency="props.baseCurrency" />
                <span v-else>—</span>
              </dd>
            </div>
            <div class="col-span-2">
              <dt class="text-xs text-muted-foreground">{{ t('pos.pnl') }}</dt>
              <dd>
                <PnlText
                  v-if="props.position.avg_cost"
                  :amount="props.position.unrealized_base"
                  :currency="props.baseCurrency"
                  :decimals="2"
                  :pct="pnlPct"
                />
                <span v-else class="text-muted-foreground">{{ t('assetDetails.noCost') }}</span>
              </dd>
            </div>
          </dl>

          <div class="flex flex-wrap gap-2">
            <AssetRevalueButton v-if="props.revaluable" :asset="asset" :store-id="props.storeId" />
            <RouterLink :to="txLink" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
              {{ t('assetDetails.history') }}
            </RouterLink>
            <RouterLink :to="analyticsLink" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
              {{ t('assetDetails.analytics') }}
            </RouterLink>
          </div>

          <AssetElsewhere :asset="asset" :store-id="props.storeId" :decimals="props.decimals" />

          <div v-if="isCrypto" class="h-80">
            <TradingChart :symbol="symbol" can-alert :limit="200" timeframe="1d" @alert="alertAt" />
          </div>

          <section v-if="isCrypto" class="grid gap-2">
            <h3 class="heading-card">{{ t('assetDetails.alerts') }}</h3>
            <PriceAlertList :asset="asset" :preset="preset" />
          </section>
        </div>

        <PriceAlertDialog v-model:open="alertOpen" :preset="chartPreset" />
      </template>
    </SheetContent>
  </Sheet>
</template>
