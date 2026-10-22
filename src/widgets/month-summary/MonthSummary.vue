<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Money } from '@/entities/currency'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { useMonthPace } from '@/shared/api/queries'
import { comparisonDelta } from '@/shared/lib/comparison'
import { formatMoney } from '@/shared/lib/format'
import { MetricTile } from '@/shared/ui/metric-tile'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

import MonthGrowth from './ui/MonthGrowth.vue'

/**
 * «Итоги месяца» (plan H3): S — saved against the previous month to the same
 * date; M — income, spending and savings; L — the plan breakdown and the
 * categories that grew most.
 */
const { t } = useI18n()
const size = useWidgetSize()

const summary = useQuery({ queryKey: ['month-summary'], queryFn: () => call<MonthSummary>('get_month_summary', { month: null }) })
const pace = useMonthPace()
const data = computed(() => summary.data.value ?? null)
const money = (value: string | undefined) => formatMoney(value, { decimals: 0, currency: data.value?.currency })

const tiles = computed(() => {
  const value = pace.data.value
  if (!value) return []
  const all = [
    { key: 'saved', cmp: value.saved, good: 'up' as const },
    { key: 'income', cmp: value.income, good: 'up' as const },
    { key: 'spent', cmp: value.spent, good: 'down' as const },
  ]
  return (size.value === 's' ? all.slice(0, 1) : all).map((tile) => ({ ...tile, ...comparisonDelta(tile.cmp) }))
})

const rows = computed(() => {
  const value = data.value
  if (!value) return []
  return [
    { key: 'fixed', amount: value.fixed },
    { key: 'daily', amount: value.daily },
    { key: 'savingsPlan', amount: value.savings_plan },
    { key: 'savingsFact', amount: value.savings_fact },
  ]
})
</script>

<template>
  <Card size="sm">
    <template #header>
      <p data-widget-title class="text-sm text-muted-foreground">{{ t('monthSummary.title') }}</p>
    </template>
    <SectionState :loading="summary.isPending.value" :error="summary.error.value" skeleton="figure" compact @retry="summary.refetch()">
      <div v-if="data" class="grid gap-3" data-tour="dash-month-summary">
        <div class="grid gap-3" :class="size !== 's' && 'grid-cols-3'">
          <MetricTile
            v-for="(tile, index) in tiles"
            :key="tile.key"
            :label="t(`monthSummary.tile.${tile.key}`)"
            :delta-pct="tile.pct"
            :good-when="tile.good"
            :base-label="index === 0 ? t('dashboard.base.prev_month') : undefined"
            :action="index === 0 ? { label: t('monthSummary.open'), to: { name: 'money-where' } } : undefined"
            size="s"
          >
            <Money :amount="tile.cmp.value" :currency="pace.data.value?.currency" :decimals="0" />
          </MetricTile>
        </div>
        <template v-if="size === 'l'">
          <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
            <template v-for="row in rows" :key="row.key">
              <dt class="text-muted-foreground">{{ t(`monthSummary.${row.key}`) }}</dt>
              <dd class="text-right"><Money :amount="row.amount" :decimals="0" :currency="data.currency" /></dd>
            </template>
            <dt class="text-muted-foreground">{{ t('monthSummary.savedPct') }}</dt>
            <dd class="text-right" :class="Number(data.deviation) < 0 ? 'text-destructive' : 'text-success'">
              {{ data.saved_pct }}%
            </dd>
          </dl>
          <MonthGrowth v-if="pace.data.value" :growth="pace.data.value.growth" :currency="pace.data.value.currency" />
        </template>
        <p v-if="data.not_executed" class="text-xs text-destructive">
          {{ t('monthSummary.notExecuted', { count: data.not_executed }) }}
        </p>
        <p v-if="size !== 's' && (Number(data.budget_in) || Number(data.budget_out))" class="text-xs text-muted-foreground">
          {{ t('monthSummary.moves', { in: money(data.budget_in), out: money(data.budget_out) }) }}
        </p>
      </div>
    </SectionState>
  </Card>
</template>
