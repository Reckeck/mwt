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

const props = defineProps<{ report: PnlReport }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))

const option = computed<ChartOption>(() => ({
  ...categoryChart(
    props.report.rows.map((row) => monthLabel(row.month, activeLocale.value)),
    axisColor.value,
    moneyTooltip.value,
  ),
  series: [
    { name: t('report.pnl.realized'), type: 'bar', data: props.report.rows.map((row) => num(row.realized)) },
    { name: t('report.pnl.fees'), type: 'bar', data: props.report.rows.map((row) => -num(row.fees)) },
  ],
}))
</script>

<template>
  <div class="grid gap-4">
    <div class="grid gap-4 @sm:grid-cols-2">
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('report.pnl.realized') }}</p>
        <p
          class="text-xl font-semibold tabular-nums"
          :class="Number(props.report.total_realized) < 0 ? 'text-destructive' : 'text-emerald-600 dark:text-emerald-400'"
        >
          {{ formatMoney(props.report.total_realized, { currency: props.report.meta.currency }) }}
        </p>
      </Card>
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('report.pnl.fees') }}</p>
        <p class="text-xl font-semibold tabular-nums">
          {{ formatMoney(props.report.total_fees, { currency: props.report.meta.currency }) }}
        </p>
      </Card>
    </div>
    <Card>
      <div class="h-72 w-full"><Chart :option="option" autoresize /></div>
    </Card>
  </div>
</template>
