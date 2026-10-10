<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AllocationPie } from '@/widgets/allocation-pie'
import { PortfolioEvolution } from '@/widgets/portfolio-evolution'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatDate } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'

import { categoryChart, num } from './chart'

const props = defineProps<{ report: PortfolioReport }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)

/** Share of each asset over time, stacked to 100 %. */
const option = computed<ChartOption>(() => {
  const base = categoryChart(
    props.report.points.map((point) => formatDate(point.date)),
    axisColor.value,
  )
  return {
    ...base,
    yAxis: { ...(base.yAxis as object), max: 100, axisLabel: { color: axisColor.value, formatter: '{value}%' } },
    series: props.report.assets.map((symbol, index) => ({
      name: symbol || t('report.portfolio.other'),
      type: 'line' as const,
      stack: 'share',
      areaStyle: { opacity: 0.5 },
      showSymbol: false,
      data: props.report.points.map((point) => num(point.shares[index])),
    })),
  }
})
</script>

<template>
  <div class="grid gap-4 @lg:grid-cols-[2fr_1fr]">
    <div class="@lg:col-span-2">
      <PortfolioEvolution />
    </div>
    <Card>
      <template #header>
        <p class="font-medium">{{ t('report.portfolio.title') }}</p>
      </template>
      <div v-if="props.report.points.length" class="h-80 w-full"><Chart :option="option" autoresize /></div>
      <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t('report.empty') }}</p>
    </Card>
    <AllocationPie />
  </div>
</template>
