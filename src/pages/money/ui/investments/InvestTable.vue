<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { formatQuantity, signClass } from '@/shared/lib/format'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { investMoney } from '../../lib/invest-view'

/** «Актив · Количество · Вложено · Прибыль» (Money 2 · 13); «актив количество прибыль» on a phone. */
const props = defineProps<{ rows: InvestmentRow[]; currency: string }>()

const { t } = useI18n()
const money = (value: string, signed = false) => investMoney(value, props.currency, signed)
/** A coin counts in itself; a futures margin names its currency. */
const quantity = (row: InvestmentRow) => (row.asset ? formatQuantity(row.quantity, 4) : `${formatQuantity(row.quantity, 2)} ${row.unit}`)
const key = (row: InvestmentRow) => row.store_id ?? row.label
</script>

<template>
  <div class="rounded-xl border bg-card shadow-sm" data-tour="invest-table">
    <Table class="max-md:hidden">
      <TableHeader>
        <TableRow>
          <TableHead>{{ t('money.invest.asset') }}</TableHead>
          <TableHead class="text-right">{{ t('money.invest.quantity') }}</TableHead>
          <TableHead class="text-right">{{ t('money.invest.invested') }}</TableHead>
          <TableHead class="text-right">{{ t('money.invest.profit') }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in props.rows" :key="key(row)" data-invest-row>
          <TableCell class="font-medium">{{ row.label }}</TableCell>
          <TableCell class="text-right tabular-nums">{{ quantity(row) }}</TableCell>
          <TableCell class="text-right tabular-nums">{{ money(row.invested) }}</TableCell>
          <TableCell class="text-right tabular-nums" :class="signClass(row.profit)">
            {{ money(row.profit, true) }}<span v-if="!row.cost_known" class="text-muted-foreground" :title="t('money.invest.costUnknown')">*</span>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    <ul class="divide-y md:hidden">
      <li v-for="row in props.rows" :key="key(row)" class="flex items-baseline gap-3 px-4 py-3 text-sm">
        <b class="min-w-0 truncate">{{ row.label }}</b>
        <span class="tabular-nums text-muted-foreground">{{ quantity(row) }}</span>
        <b class="ms-auto shrink-0 tabular-nums" :class="signClass(row.profit)">{{ money(row.profit, true) }}</b>
      </li>
    </ul>
    <p v-if="props.rows.some((row) => !row.cost_known)" class="border-t px-4 py-2 text-xs text-muted-foreground">* {{ t('money.invest.costUnknown') }}</p>
  </div>
</template>
