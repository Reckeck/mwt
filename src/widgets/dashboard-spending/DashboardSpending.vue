<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { comparisonDelta } from '@/shared/lib/comparison'
import { Card } from '@/shared/ui/card'
import { MetricTile } from '@/shared/ui/metric-tile'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

import type { DashboardSpendingProps } from './dashboard-spending.types'
import { useSpendingOverview } from './model/use-spending-overview'
import SpendingMonth from './ui/SpendingMonth.vue'
import SpendingWeek from './ui/SpendingWeek.vue'

/**
 * «Траты» (plan H2: the 7-day bars and the mini calendar in one): S — the week
 * against the one before; M — the period against the previous one, the bars and
 * the trend; L — the month's calendar too. Rising spending is the bad direction.
 */
const props = defineProps<DashboardSpendingProps>()
const { t } = useI18n()
const size = useWidgetSize()

const overview = useSpendingOverview(() => props.period)
const data = computed(() => overview.data.value)
const week = computed(() => comparisonDelta(data.value?.week_total))
const spent = computed(() => comparisonDelta(data.value?.spent))
</script>

<template>
  <Card size="sm">
    <SectionState :loading="overview.isPending.value" :error="overview.error.value" skeleton="figure" compact @retry="overview.refetch()">
      <div v-if="data" class="flex flex-col gap-3">
        <div class="grid gap-3" :class="size !== 's' && 'grid-cols-2'">
          <MetricTile
            :label="t('dashboard.spending.week')"
            :delta-pct="week.pct"
            good-when="down"
            :base-label="t('dashboard.base.prevWeek')"
          >
            <Money :amount="data.week_total.value" :currency="data.currency" :decimals="0" />
            <template #delta>
              <Money :amount="week.delta" :currency="data.currency" :decimals="0" signed :convert="false" />
            </template>
          </MetricTile>
          <MetricTile
            v-if="size !== 's'"
            :label="t(`dashboard.spending.period_${data.period}`)"
            :delta-pct="spent.pct"
            good-when="down"
            :base-label="t(`dashboard.base.prev_${data.period}`)"
            :series="data.series.map(Number)"
          >
            <Money :amount="data.spent.value" :currency="data.currency" :decimals="0" />
            <template #delta>
              <Money :amount="spent.delta" :currency="data.currency" :decimals="0" signed :convert="false" />
            </template>
          </MetricTile>
        </div>
        <SpendingWeek v-if="size !== 's'" :days="data.week" :currency="data.currency" :allowance="data.allowance" />
        <SpendingMonth v-if="size === 'l'" />
      </div>
    </SectionState>
  </Card>
</template>
