<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney, formatPct, signClass } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { monthLabel } from '../use-report'

const props = defineProps<{ report: CategoriesReport }>()

const { t } = useI18n()
const categories = useQuery({ queryKey: ['categories'], queryFn: () => call<Category[]>('list_categories') })
const label = (key: string) => (key ? categoryKeyLabel(key, categories.data.value, t) : t('report.categories.none'))
const money = (value: string) => formatMoney(value, { decimals: 0 })

/** Cell shade by its share of the group's biggest cell — a heat map without a chart. */
function heat(block: CategoryGroupBlock, value: string): string {
  const max = Math.max(...block.rows.flatMap((row) => row.cells.map(Number)), 0)
  const ratio = max > 0 ? Number(value) / max : 0
  return ratio > 0 ? `color-mix(in oklab, var(--primary) ${Math.round(ratio * 45)}%, transparent)` : 'transparent'
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <Card v-if="props.report.top.length">
      <template #header>
        <p class="font-medium">{{ t('report.categories.top') }}</p>
      </template>
      <ol class="grid gap-1.5 text-sm">
        <li v-for="(row, index) in props.report.top" :key="row.key" class="flex items-center gap-3">
          <span class="w-4 text-muted-foreground">{{ index + 1 }}</span>
          <span class="min-w-0 flex-1 truncate">{{ label(row.key) }}</span>
          <span class="tabular-nums">{{ money(row.total) }}</span>
          <span
            v-if="row.delta_pct !== null"
            class="w-16 text-right text-xs tabular-nums"
            :class="signClass(-Number(row.delta_pct))"
          >
            {{ formatPct(row.delta_pct, { signed: true, decimals: 1 }) }}
          </span>
          <span v-else class="w-16" />
        </li>
      </ol>
    </Card>

    <Card v-for="block in props.report.groups" :key="block.group ?? 'none'">
      <template #header>
        <div class="flex items-center justify-between gap-2">
          <p class="font-medium">{{ t(`categoryGroup.${block.group ?? 'daily'}`) }}</p>
          <span class="text-sm tabular-nums text-muted-foreground">{{ money(block.total) }}</span>
        </div>
      </template>
      <div class="overflow-x-auto">
        <Table class="w-full text-sm tabular-nums">
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="text-left">{{ t('report.categories.category') }}</TableHead>
              <TableHead v-for="month in props.report.meta.months" :key="month" class="text-right capitalize">
                {{ monthLabel(month, activeLocale) }}
              </TableHead>
              <TableHead class="text-right">{{ t('report.total') }}</TableHead>
              <TableHead class="text-right">{{ t('report.categories.average') }}</TableHead>
              <TableHead class="text-right">{{ t('report.categories.share') }}</TableHead>
              <TableHead class="text-right">Δ</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="row in block.rows" :key="row.key">
              <TableCell class="max-w-48 truncate">{{ label(row.key) }}</TableCell>
              <TableCell
                v-for="(cell, index) in row.cells"
                :key="index"
                class="text-right"
                :style="{ background: heat(block, cell) }"
              >
                {{ Number(cell) ? money(cell) : '' }}
              </TableCell>
              <TableCell class="text-right font-medium">{{ money(row.total) }}</TableCell>
              <TableCell class="text-right">{{ money(row.average) }}</TableCell>
              <TableCell class="text-right">{{ row.share }}%</TableCell>
              <TableCell class="text-right text-xs">{{ row.delta_pct === null ? '—' : `${row.delta_pct}%` }}</TableCell>
            </TableRow>
            <TableRow class="font-medium">
              <TableCell>{{ t('report.total') }}</TableCell>
              <TableCell v-for="(cell, index) in block.totals" :key="index" class="text-right">{{ money(cell) }}</TableCell>
              <TableCell class="text-right">{{ money(block.total) }}</TableCell>
              <TableCell colspan="3" />
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </Card>
    <p v-if="!props.report.groups.length" class="py-12 text-center text-sm text-muted-foreground">{{ t('report.empty') }}</p>
  </div>
</template>
