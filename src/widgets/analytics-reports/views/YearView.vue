<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'

import { monthLabel } from '../use-report'

const props = defineProps<{ report: YearReport }>()

const { t } = useI18n()
const money = (value: string) => formatMoney(value, { decimals: 0 })
const columns = ['income', 'fixed', 'daily', 'savings_plan', 'savings_fact', 'deviation'] as const
</script>

<template>
  <div class="grid gap-4">
    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-4">
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('report.year.income') }}</p>
        <p class="text-xl font-semibold tabular-nums">{{ money(props.report.total.income) }}</p>
      </Card>
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('report.year.saved') }}</p>
        <p class="text-xl font-semibold tabular-nums">
          {{ money(props.report.total.savings_fact) }}
          <span class="text-sm font-normal text-muted-foreground">· {{ props.report.total.saved_pct }}%</span>
        </p>
      </Card>
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('report.year.best') }}</p>
        <p class="text-xl font-semibold capitalize">
          {{ props.report.best_month ? monthLabel(props.report.best_month, activeLocale) : '—' }}
        </p>
      </Card>
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('report.year.worst') }}</p>
        <p class="text-xl font-semibold capitalize">
          {{ props.report.worst_month ? monthLabel(props.report.worst_month, activeLocale) : '—' }}
        </p>
      </Card>
    </div>
    <Card>
      <div class="overflow-x-auto">
        <table class="w-full text-sm tabular-nums">
          <thead class="text-xs text-muted-foreground">
            <tr>
              <th class="py-1 pr-2 text-left font-normal">{{ t('report.month') }}</th>
              <th v-for="column in columns" :key="column" class="px-2 py-1 text-right font-normal">
                {{ t(`report.year.${column}`) }}
              </th>
              <th class="py-1 pl-2 text-right font-normal">%</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="row in props.report.rows"
              :key="row.month"
              class="border-t border-border"
              :class="row.empty && 'text-muted-foreground'"
            >
              <td class="py-1.5 pr-2 capitalize">{{ monthLabel(row.month, activeLocale) }}</td>
              <td
                v-for="column in columns"
                :key="column"
                class="px-2 py-1.5 text-right"
                :class="column === 'deviation' && !row.empty && (Number(row.deviation) < 0 ? 'text-destructive' : 'text-emerald-600 dark:text-emerald-400')"
              >
                {{ row.empty ? '—' : money(row[column]) }}
              </td>
              <td class="py-1.5 pl-2 text-right">{{ row.empty ? '—' : `${row.saved_pct}%` }}</td>
            </tr>
            <tr v-for="(row, label) in { total: props.report.total, average: props.report.average }" :key="label" class="border-t-2 border-border font-medium">
              <td class="py-1.5 pr-2">{{ t(`report.year.${label}`) }}</td>
              <td v-for="column in columns" :key="column" class="px-2 py-1.5 text-right">{{ money(row[column]) }}</td>
              <td class="py-1.5 pl-2 text-right">{{ row.saved_pct }}%</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p class="mt-3 text-xs text-muted-foreground">{{ t('report.year.averageNote') }}</p>
    </Card>
  </div>
</template>
