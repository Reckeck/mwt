<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMonthKey } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'

import { percent } from '../lib/reason-text'
import { savingsChart } from '../model/savings-chart'

/** The savings rate month by month against the plan, with the 20% benchmark. */
const props = defineProps<{ report: HealthReport }>()
const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)

const rows = computed(() => props.report.savings)
const known = computed(() => rows.value.some((row) => row.saved_pct !== null))
const option = computed<ChartOption>(() =>
  savingsChart(
    rows.value,
    props.report.savings_target_pct,
    {
      month: (month) => formatMonthKey(month, { month: 'short' }),
      saved: t('health.savings.saved'),
      target: t('health.savings.target', { pct: percent(props.report.savings_target_pct) }),
    },
    {
      axisColor: axisColor.value,
      tooltip: (params) => {
        const [point] = params as { name: string; value: number | null }[]
        return point ? `${point.name}: ${point.value === null ? '—' : percent(String(point.value))}` : ''
      },
    },
  ),
)
</script>

<template>
  <Card data-tour="health-savings">
    <template #header>
      <p class="font-medium">{{ t('health.savings.title') }}</p>
    </template>
    <EmptyState v-if="!known" compact :title="t('health.savings.emptyTitle')" :description="t('health.savings.empty')" />
    <div v-else class="grid gap-3">
      <p class="text-sm">
        {{
          props.report.savings_avg_pct === null
            ? t('health.savings.noAverage')
            : t('health.savings.average', {
                pct: percent(props.report.savings_avg_pct),
                target: percent(props.report.savings_target_pct),
              })
        }}
      </p>
      <Chart :option="option" class="h-48" />
      <p class="text-xs text-muted-foreground">{{ t('health.savings.benchmark') }}</p>
    </div>
  </Card>
</template>
