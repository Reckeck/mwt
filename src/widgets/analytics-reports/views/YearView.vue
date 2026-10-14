<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { formatMoney, signClass } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { monthLabel } from '../use-report'

const props = defineProps<{ report: YearReport }>()

const { t } = useI18n()
const money = (value: string) => formatMoney(value, { decimals: 0 })
const columns = ['income', 'fixed', 'daily', 'savings_plan', 'savings_fact', 'deviation'] as const
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
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
        <Table class="w-full text-sm tabular-nums">
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="text-left">{{ t('report.month') }}</TableHead>
              <TableHead v-for="column in columns" :key="column" class="text-right">
                {{ t(`report.year.${column}`) }}
              </TableHead>
              <TableHead class="text-right">%</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="row in props.report.rows"
              :key="row.month"
              :class="row.empty && 'text-muted-foreground'"
            >
              <TableCell class="capitalize">{{ monthLabel(row.month, activeLocale) }}</TableCell>
              <TableCell
                v-for="column in columns"
                :key="column"
                class="text-right"
                :class="column === 'deviation' && !row.empty && signClass(row.deviation)"
              >
                {{ row.empty ? '—' : money(row[column]) }}
              </TableCell>
              <TableCell class="text-right">{{ row.empty ? '—' : `${row.saved_pct}%` }}</TableCell>
            </TableRow>
            <TableRow v-for="(row, label) in { total: props.report.total, average: props.report.average }" :key="label" class="font-medium">
              <TableCell>{{ t(`report.year.${label}`) }}</TableCell>
              <TableCell v-for="column in columns" :key="column" class="text-right">{{ money(row[column]) }}</TableCell>
              <TableCell class="text-right">{{ row.saved_pct }}%</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
      <p class="mt-3 text-xs text-muted-foreground">{{ t('report.year.averageNote') }}</p>
    </Card>
  </div>
</template>
