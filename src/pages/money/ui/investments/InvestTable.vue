<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { formatQuantity, signClass } from '@/shared/lib/format'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { investMoney } from '../../lib/invest-view'

/** «Актив · Количество · Вложено · Прибыль» (Money 2 · 13); on a phone the asset over «количество · вложено», the profit on the right. */
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
          <TableCell class="text-right tabular-nums"><Money v-bind="money(row.invested)" /></TableCell>
          <TableCell class="text-right tabular-nums" :class="signClass(row.profit)">
            <Money v-bind="money(row.profit, true)" /><span v-if="!row.cost_known" class="text-muted-foreground" :title="t('money.invest.costUnknown')">*</span>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    <ul class="divide-y md:hidden">
      <li v-for="row in props.rows" :key="key(row)" class="flex items-center gap-3 px-4 py-3 text-sm">
        <span class="grid min-w-0 flex-1 gap-0.5">
          <b class="truncate font-medium">{{ row.label }}</b>
          <span class="truncate text-xs tabular-nums text-muted-foreground">
            {{ quantity(row) }} · {{ t('money.invest.investedShort') }} <Money v-bind="money(row.invested)" />
          </span>
        </span>
        <b class="shrink-0 tabular-nums" :class="signClass(row.profit)">
          <Money v-bind="money(row.profit, true)" /><span v-if="!row.cost_known" class="text-muted-foreground">*</span>
        </b>
      </li>
    </ul>
    <p v-if="props.rows.some((row) => !row.cost_known)" class="border-t px-4 py-2 text-xs text-muted-foreground">* {{ t('money.invest.costUnknown') }}</p>
  </div>
</template>
