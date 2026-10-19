<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'

import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: InflationReport }>()

const { t } = useI18n()
const route = useRoute()
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))

const option = computed<ChartOption>(() => ({
  ...categoryChart(
    props.report.points.map((point) => monthLabel(point.month, activeLocale.value)),
    axisColor.value,
    moneyTooltip.value,
    props.report.meta.currency,
  ),
  series: [
    { name: t('report.inflation.nominal'), type: 'line', data: props.report.points.map((point) => num(point.nominal_net)) },
    { name: t('report.inflation.real'), type: 'line', data: props.report.points.map((point) => num(point.real_net)) },
  ],
}))
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <Card>
      <div class="grid gap-1 text-sm">
        <p v-if="props.report.meta.inflation_rate">
          {{ t('report.inflation.rate', { rate: props.report.meta.inflation_rate, currency: props.report.meta.currency.toUpperCase() }) }}
        </p>
        <p v-else class="text-warning">
          {{ t('report.inflation.noRate', { currency: props.report.meta.currency.toUpperCase() }) }}
        </p>
        <p v-if="props.report.nominal_change_pct !== null" class="tabular-nums">
          {{ t('report.inflation.change', { nominal: props.report.nominal_change_pct, real: props.report.real_change_pct ?? '—' }) }}
        </p>
        <p class="text-xs text-muted-foreground">
          {{ t('report.inflation.reference') }}
          <RouterLink :to="{ query: { ...route.query, settings: 'inflation' } }" class="underline">{{ t('report.inflation.settings') }}</RouterLink>
        </p>
      </div>
    </Card>
    <Card>
      <div class="h-80 w-full"><Chart :option="option" autoresize /></div>
    </Card>
  </div>
</template>
