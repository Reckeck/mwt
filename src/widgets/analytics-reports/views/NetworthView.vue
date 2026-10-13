<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'

import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: NetworthReport }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))
const money = (value: string) => formatMoney(value, { decimals: 0 })

const PARTS = ['financial', 'non_financial', 'receivables'] as const
const DEBTS = ['liabilities', 'payables'] as const

const option = computed<ChartOption>(() => ({
  ...categoryChart(
    props.report.points.map((point) => monthLabel(point.month, activeLocale.value)),
    axisColor.value,
    moneyTooltip.value,
  ),
  series: [
    ...PARTS.map((part) => ({
      name: t(`report.networth.${part}`),
      type: 'line' as const,
      stack: 'assets',
      areaStyle: { opacity: 0.25 },
      showSymbol: false,
      data: props.report.points.map((point) => num(point[part])),
    })),
    ...DEBTS.map((part) => ({
      name: t(`report.networth.${part}`),
      type: 'line' as const,
      stack: 'debts',
      areaStyle: { opacity: 0.25 },
      showSymbol: false,
      data: props.report.points.map((point) => -num(point[part])),
    })),
    {
      name: t('report.networth.net'),
      type: 'line' as const,
      lineStyle: { width: 3 },
      data: props.report.points.map((point) => num(point.net)),
    },
  ],
}))
const last = computed(() => props.report.points[props.report.points.length - 1] ?? null)
</script>

<template>
  <div class="grid gap-4">
    <Card>
      <div class="h-80 w-full"><Chart :option="option" autoresize /></div>
    </Card>
    <Card v-if="last">
      <dl class="grid grid-cols-2 gap-x-6 gap-y-1.5 text-sm tabular-nums @md:grid-cols-3">
        <template v-for="part in [...PARTS, ...DEBTS, 'net' as const]" :key="part">
          <dt class="text-muted-foreground">{{ t(`report.networth.${part}`) }}</dt>
          <dd class="text-right" :class="part === 'net' && 'font-semibold'">{{ money(last[part]) }}</dd>
        </template>
      </dl>
    </Card>
  </div>
</template>
