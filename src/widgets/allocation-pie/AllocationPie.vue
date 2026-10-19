<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMoney } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'

import type { AllocationPieProps } from './allocation-pie.types'

/** Slices a pie keeps; the rest are one «Other» slice, or the labels collide. */
const MAX_SLICES = 7

/** Where the money is, as a ring: by asset on a wallet, by network or wallet on the dashboard. */
const props = withDefaults(defineProps<AllocationPieProps>(), { title: '', bare: false, currency: undefined })
const { t } = useI18n()
const { resolved } = useTheme()
const axisColor = chartAxisColor(resolved)

const data = computed(() => {
  const sorted = props.slices
    .map((slice) => ({ name: slice.label, value: Number(slice.value) }))
    .filter((slice) => slice.value > 0)
    .sort((left, right) => right.value - left.value)
  if (sorted.length <= MAX_SLICES) return sorted
  const rest = sorted.slice(MAX_SLICES - 1).reduce((sum, slice) => sum + slice.value, 0)
  return [...sorted.slice(0, MAX_SLICES - 1), { name: t('charts.other'), value: rest }]
})

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  tooltip: {
    trigger: 'item',
    // Chart tooltips are text: the hover with conversions belongs to the figures next to it.
    formatter: (params) => {
      const item = params as unknown as { name: string; value: number; percent: number }
      return `${item.name}: ${formatMoney(item.value, { currency: props.currency })} (${item.percent}%)`
    },
  },
  series: [
    {
      name: props.title || t('charts.distribution'),
      type: 'pie',
      radius: ['45%', '72%'],
      center: ['50%', '50%'],
      avoidLabelOverlap: true,
      itemStyle: { borderRadius: 6, borderColor: 'transparent', borderWidth: 2 },
      label: { color: axisColor.value, formatter: '{b}\n{d}%' },
      data: data.value,
    },
  ],
}))
</script>

<template>
  <!-- The chart fills its box: vue-echarts forces height: 100% on its own element,
       so a height class on <Chart> is ignored and the canvas ends up 0px tall. -->
  <div v-if="props.bare" class="h-56 w-full"><Chart :option="option" autoresize /></div>
  <Card v-else>
    <template #header>
      <h3 class="heading-card">{{ props.title || t('charts.distribution') }}</h3>
    </template>
    <div class="h-64 w-full"><Chart :option="option" autoresize /></div>
  </Card>
</template>
