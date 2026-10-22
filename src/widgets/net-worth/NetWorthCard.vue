<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { comparisonDelta } from '@/shared/lib/comparison'
import { Card } from '@/shared/ui/card'
import { MetricTile } from '@/shared/ui/metric-tile'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

import { useNetWorthTrend } from './model/use-net-worth-trend'
import type { NetWorthCardProps } from './net-worth.types'
import NetWorthBreakdown from './ui/NetWorthBreakdown.vue'

/**
 * «Чистый капитал» (plan H3): S — the figure and its change over the period;
 * M — the trend too; L — the breakdown by kind, basket and loan.
 */
const props = withDefaults(defineProps<NetWorthCardProps>(), { period: 'month' })
const { t } = useI18n()
const size = useWidgetSize()

const worth = useQuery({ queryKey: ['net-worth'], queryFn: () => call<NetWorth>('get_net_worth') })
const trend = useNetWorthTrend(() => props.period)
const change = computed(() => comparisonDelta(trend.data.value?.change))
</script>

<template>
  <Card size="sm">
    <SectionState :loading="worth.isPending.value" :error="worth.error.value" skeleton="figure" compact @retry="worth.refetch()">
      <div v-if="worth.data.value" class="grid gap-3" data-tour="dash-net-worth">
        <MetricTile
          :label="t('netWorthCard.title')"
          :delta-pct="change.pct"
          good-when="up"
          :base-label="t(`netWorthCard.since_${period}`)"
          :series="trend.data.value?.series.map(Number)"
          :action="{ label: t('netWorthCard.open'), to: { name: 'money-wealth' } }"
        >
          <Money :amount="worth.data.value.net" :currency="worth.data.value.base_currency" :decimals="0" />
          <template #delta>
            <Money
              :amount="change.delta"
              :currency="trend.data.value?.currency ?? worth.data.value.base_currency"
              :decimals="0"
              signed
              :convert="false"
            />
          </template>
        </MetricTile>
        <NetWorthBreakdown v-if="size === 'l'" :worth="worth.data.value" />
      </div>
    </SectionState>
  </Card>
</template>
