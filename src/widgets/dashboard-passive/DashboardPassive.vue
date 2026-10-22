<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { percent } from '@/widgets/capital-health'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY } from '@/shared/api/queries'
import { Card } from '@/shared/ui/card'
import { MetricTile } from '@/shared/ui/metric-tile'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

/**
 * «Пассивный доход» (plan H4): S — the month so far; M — the trend of the last
 * months and how much of the base spending it covers; L — the year's total too.
 */
const { t } = useI18n()
const size = useWidgetSize()

const report = useQuery({
  queryKey: CAPITAL_HEALTH_KEY,
  queryFn: () => call<HealthReport>('capital_health'),
  placeholderData: keepPreviousData,
})
const passive = computed(() => report.data.value?.passive ?? null)

/** The last finished month against the one before it. */
const changePct = computed(() => {
  const months = passive.value?.months ?? []
  const last = Number(months.at(-1)?.amount ?? 0)
  const prev = Number(months.at(-2)?.amount ?? 0)
  return months.length > 1 && prev > 0 ? ((last - prev) / prev) * 100 : null
})

const insight = computed(() => {
  const pct = passive.value?.freedom_fact_pct
  return pct === null || pct === undefined ? t('health.freedom.noBase') : t('health.freedom.fact', { pct: percent(pct) })
})
</script>

<template>
  <Card size="sm">
    <SectionState
      :loading="report.isPending.value"
      :error="report.error.value"
      skeleton="figure"
      compact
      @retry="report.refetch()"
    >
      <div v-if="passive" class="grid gap-3" data-tour="dash-passive">
        <MetricTile
          :label="t('dashboard.passive.thisMonth')"
          :delta-pct="changePct"
          good-when="up"
          :base-label="t('dashboard.passive.base')"
          :series="passive.months.map((month) => month.amount)"
          :insight="insight"
          :action="{ label: t('dashboard.health.open'), to: { name: 'money-health' } }"
        >
          <Money :amount="passive.this_month" :currency="passive.currency" :decimals="0" :convert="false" />
        </MetricTile>
        <p v-if="size === 'l'" class="flex flex-wrap justify-between gap-2 text-xs text-muted-foreground">
          <span>{{ t('health.freedom.total') }}</span>
          <Money :amount="passive.total_12m" :currency="passive.currency" :decimals="0" :convert="false" />
        </p>
      </div>
    </SectionState>
  </Card>
</template>
