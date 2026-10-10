<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'

const { t } = useI18n()

const summary = useQuery({ queryKey: ['month-summary'], queryFn: () => call<MonthSummary>('get_month_summary', { month: null }) })
const data = computed(() => summary.data.value ?? null)
const money = (value: string | undefined) => formatMoney(value, { decimals: 0, currency: data.value?.currency })

const rows = computed(() => {
  const value = data.value
  if (!value) return []
  return [
    { key: 'income', amount: value.income },
    { key: 'fixed', amount: value.fixed },
    { key: 'daily', amount: value.daily },
    { key: 'savingsPlan', amount: value.savings_plan },
    { key: 'savingsFact', amount: value.savings_fact },
  ]
})
</script>

<template>
  <Card v-if="data">
    <template #header>
      <p class="text-sm text-muted-foreground">{{ t('monthSummary.title') }}</p>
    </template>
    <dl class="grid grid-cols-2 gap-x-4 gap-y-1.5 text-sm tabular-nums">
      <template v-for="row in rows" :key="row.key">
        <dt class="text-muted-foreground">{{ t(`monthSummary.${row.key}`) }}</dt>
        <dd class="text-right">{{ money(row.amount) }}</dd>
      </template>
      <dt class="text-muted-foreground">{{ t('monthSummary.savedPct') }}</dt>
      <dd class="text-right" :class="Number(data.deviation) < 0 ? 'text-destructive' : 'text-emerald-600 dark:text-emerald-400'">
        {{ data.saved_pct }}%
      </dd>
    </dl>
    <p v-if="data.not_executed" class="mt-3 text-xs text-destructive">
      {{ t('monthSummary.notExecuted', { count: data.not_executed }) }}
    </p>
    <p v-if="Number(data.budget_in) || Number(data.budget_out)" class="mt-1 text-xs text-muted-foreground">
      {{ t('monthSummary.moves', { in: money(data.budget_in), out: money(data.budget_out) }) }}
    </p>
  </Card>
</template>
