<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
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
      data: slices.value.map((row) => ({ name: row.label, value: Number(row.value_usd) })),
    },
  ],
}))
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('charts.distribution') }}</p>
    </template>
    <Chart v-if="slices.length > 0" :option="option" autoresize class="h-80 w-full" />
    <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t('store.empty') }}</p>
  </Card>
</template>
