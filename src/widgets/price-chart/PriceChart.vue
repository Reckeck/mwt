<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { chartColors } from '@/shared/lib/chart-theme'
import { Chart } from '@/shared/lib/echarts'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { candleOption } from './lib/candle-option'
import { useCandles } from './model/use-candles'

/**
 * The price of a coin: closing price over the chosen step, drawn with the chart kit (`--chart-*` colours, so both
 * themes follow). Read-only market data; nothing here writes.
 */
const props = withDefaults(defineProps<{ symbols?: string[]; currency?: string }>(), {
  symbols: () => ['BTC', 'ETH'],
  currency: 'USD',
})
const { t } = useI18n()

const STEPS = [
  { id: 'hour', minutes: 60 },
  { id: 'day', minutes: 1440 },
] as const
const symbol = ref(props.symbols[0] ?? 'BTC')
const step = ref<(typeof STEPS)[number]['id']>('hour')
const minutes = computed(() => STEPS.find((entry) => entry.id === step.value)?.minutes ?? 60)

const candles = useCandles(symbol, minutes)
const option = computed(() =>
  candleOption(chartColors(), {
    candles: candles.data.value ?? [],
    intervalMin: minutes.value,
    name: symbol.value,
    currency: props.currency,
  }),
)

function pickSymbol(value: unknown) {
  if (typeof value === 'string' && value) symbol.value = value
}
function pickStep(value: unknown) {
  if (value === 'hour' || value === 'day') step.value = value
}
</script>

<template>
  <Card class="grid gap-4 p-4 md:p-6" data-price-chart>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h2 class="text-base font-semibold">{{ t('cryptoTrack.chart.title') }}</h2>
      <div class="flex flex-wrap gap-2">
        <ToggleGroup type="single" variant="range" :spacing="1" :model-value="symbol" :aria-label="t('cryptoTrack.chart.symbol')" @update:model-value="pickSymbol">
          <ToggleGroupItem v-for="item in props.symbols" :key="item" :value="item" :data-chart-symbol="item">{{ item }}</ToggleGroupItem>
        </ToggleGroup>
        <ToggleGroup type="single" variant="range" :spacing="1" :model-value="step" :aria-label="t('cryptoTrack.chart.step')" @update:model-value="pickStep">
          <ToggleGroupItem v-for="item in STEPS" :key="item.id" :value="item.id" :data-chart-step="item.id">
            {{ t(`cryptoTrack.chart.steps.${item.id}`) }}
          </ToggleGroupItem>
        </ToggleGroup>
      </div>
    </div>
    <SectionState
      :loading="candles.isPending.value"
      :error="candles.error.value"
      :empty="(candles.data.value ?? []).length === 0"
      :empty-title="t('cryptoTrack.chart.empty')"
      skeleton="list"
      @retry="candles.refetch()"
    >
      <Chart :option="option" class="h-56 w-full md:h-72" role="img" :aria-label="t('cryptoTrack.chart.title')" data-price-chart-plot />
    </SectionState>
  </Card>
</template>
