<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { call } from '@/shared/api/ipc'
import { useTheme } from '@/shared/lib/theme'

const { t } = useI18n()
const { resolved } = useTheme()
const axisColor = chartAxisColor(resolved)

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const slices = computed(() => summary.data.value?.by_chain ?? [])

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  tooltip: { trigger: 'item', formatter: '{b}: {d}%' },
  series: [
    {
      name: t('charts.distribution'),
      type: 'pie',
      radius: ['45%', '72%'],
      center: ['50%', '50%'],
      avoidLabelOverlap: true,
      itemStyle: { borderRadius: 6, borderColor: 'transparent', borderWidth: 2 },
      label: { color: axisColor.value, formatter: '{b}\n{d}%' },
      data: slices.value.map((row) => ({ name: row.label, value: Number(row.value_base) })),
    },
  ],
}))
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('charts.distribution') }}</p>
    </template>
    <!-- The chart fills its box: vue-echarts forces height: 100% on its own element,
         so a height class on <Chart> is ignored and the canvas ends up 0px tall. -->
    <SectionState
      :loading="summary.isPending.value"
      :error="summary.error.value"
      :empty="!(slices.length > 0)"
      :empty-title="t('store.empty')"
      skeleton="chart"
      compact
      @retry="summary.refetch()"
    >
      <div class="h-80 w-full">
        <Chart :option="option" autoresize />
      </div>
    </SectionState>
  </Card>
</template>
