<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search } from 'lucide-vue-next'

import { TradingChart } from '@/widgets/trading-chart'
import { Input } from '@/shared/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

const { t } = useI18n()

const WATCHLIST = ['BTC', 'ETH', 'SOL', 'BNB', 'XRP', 'DOGE', 'ADA', 'AVAX']
const symbol = ref('BTC')
const draft = ref('BTC')

/** Accept "eth", "ETHUSDT" or "ethusd" alike: the chart always quotes against USDT. */
function submit() {
  const next = draft.value.trim().toUpperCase().replace(/USDT?$/, '')
  if (next) symbol.value = next
  else draft.value = symbol.value
}

watch(symbol, (value) => (draft.value = value))

function pick(value: unknown) {
  if (typeof value === 'string' && value) symbol.value = value
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <div class="relative">
        <Search class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          v-model="draft"
          class="h-8 w-40 pl-8 uppercase"
          :placeholder="t('trading.symbol')"
          :title="t('trading.symbolHint')"
          :aria-label="t('trading.symbol')"
          @keydown.enter="submit"
          @blur="submit"
        />
      </div>

      <ToggleGroup
        type="single"
        variant="outline"
        size="sm"
        :model-value="WATCHLIST.includes(symbol) ? symbol : ''"
        :aria-label="t('trading.watchlist')"
        @update:model-value="pick"
      >
        <ToggleGroupItem v-for="coin in WATCHLIST" :key="coin" :value="coin" class="px-2.5">
          {{ coin }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <TradingChart :symbol="symbol" class="min-h-0 flex-1" />
  </div>
</template>
