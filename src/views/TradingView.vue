<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import TradingChart from '@/components/charts/TradingChart.vue'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'

const { t } = useI18n()

const WATCHLIST = ['BTC', 'ETH', 'SOL', 'BNB', 'XRP', 'DOGE', 'ADA', 'AVAX']
const symbol = ref('BTC')
const draft = ref('BTC')

function submit() {
  const next = draft.value.trim().toUpperCase().replace(/USDT?$/, '')
  if (next) symbol.value = next
}

watch(symbol, (value) => (draft.value = value))

function pick(next: string) {
  symbol.value = next
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-4">
    <div class="flex flex-wrap items-center gap-2">
      <Input
        v-model="draft"
        class="h-8 w-32 uppercase"
        :placeholder="t('trading.symbol')"
        @keydown.enter="submit"
      />
      <Button variant="outline" size="sm" @click="submit">{{ t('trading.apply') }}</Button>

      <div class="mx-1 h-5 w-px bg-border" />

      <Button
        v-for="coin in WATCHLIST"
        :key="coin"
        variant="ghost"
        size="sm"
        class="px-2 text-xs tabular-nums"
        :class="symbol === coin && 'bg-accent text-accent-foreground'"
        @click="pick(coin)"
      >
        {{ coin }}
      </Button>
    </div>

    <TradingChart :symbol="symbol" class="min-h-0 flex-1" />
  </div>
</template>
