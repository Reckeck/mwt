<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { Money } from '@/entities/currency'

import { ReportTerm } from '@/entities/report'
import { Card } from '@/shared/ui/card'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatPct, signClass } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { transactionsOf } from '../lib/drill-down'
import { monthLabel } from '../use-report'
import { useCategories } from '@/shared/api/queries'
import { ProgressBar } from '@/shared/ui/progress-bar'

const props = defineProps<{ report: CategoriesReport; filter: ReportFilter }>()

const { t } = useI18n()
const categories = useCategories()
const label = (key: string) => (key ? categoryKeyLabel(key, categories.data.value, t) : t('report.categories.none'))
const months = computed(() => props.report.meta.months)
/** The category's transactions over the whole period, or over one month. */
const ofCategory = (key: string, month?: string) =>
  transactionsOf(
    month ? { from: month, category: key } : { from: months.value[0], to: months.value.at(-1), category: key },
    props.filter,
  )

const delta = (value: string | null) => (value === null ? '—' : formatPct(value, { signed: true, decimals: 0 }))

/** All spending of the period: the top five's shares are of this. */
const spending = computed(() =>
  props.report.groups
    .filter((block) => block.group === 'fixed' || block.group === 'daily')
    .reduce((sum, block) => sum + Number(block.total), 0),
)
const shareOf = (value: string) => (spending.value > 0 ? (Number(value) / spending.value) * 100 : 0)

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
      <ol class="grid gap-2.5 text-sm">
        <li v-for="(row, index) in props.report.top" :key="row.key" class="grid gap-1">
          <div class="flex items-center gap-3">
            <span class="w-4 text-muted-foreground">{{ index + 1 }}</span>
            <RouterLink :to="ofCategory(row.key)" class="min-w-0 flex-1 truncate hover:underline">{{ label(row.key) }}</RouterLink>
            <span class="tabular-nums"><Money :amount="row.total" :decimals="0" :currency="props.report.meta.currency" /></span>
            <span class="w-12 text-right text-xs tabular-nums text-muted-foreground">
              {{ formatPct(shareOf(row.total), { decimals: 0 }) }}
            </span>
            <span class="w-14 text-right text-xs tabular-nums" :class="row.delta_pct !== null && signClass(-Number(row.delta_pct))">
              {{ delta(row.delta_pct) }}
            </span>
          </div>
          <ProgressBar class="ml-7" :value="shareOf(row.total)" size="sm" />
        </li>
      </ol>
    </Card>

    <Card v-for="block in props.report.groups" :key="block.group ?? 'none'">
      <template #header>
        <div class="flex items-center justify-between gap-2">
          <p class="font-medium">{{ t(`categoryGroup.${block.group ?? 'daily'}`) }}</p>
          <span class="text-sm tabular-nums text-muted-foreground"><Money :amount="block.total" :decimals="0" :currency="props.report.meta.currency" /></span>
        </div>
      </template>
      <div class="overflow-x-auto">
        <Table class="w-full text-sm tabular-nums">
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="text-left sticky left-0 z-[1] bg-card">{{ t('report.categories.category') }}</TableHead>
              <TableHead class="text-right">
                <ReportTerm :label="t('report.categories.delta')" :hint="t('report.term.delta')" />
              </TableHead>
              <TableHead v-for="month in props.report.meta.months" :key="month" class="text-right capitalize">
                {{ monthLabel(month, activeLocale) }}
              </TableHead>
              <TableHead class="text-right">{{ t('report.total') }}</TableHead>
              <TableHead class="text-right">
                <ReportTerm :label="t('report.categories.average')" :hint="t('report.term.average')" />
              </TableHead>
              <TableHead class="text-right">{{ t('report.categories.share') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="row in block.rows" :key="row.key">
              <TableCell class="max-w-48 truncate sticky left-0 z-[1] bg-card">
                <RouterLink :to="ofCategory(row.key)" class="hover:underline">{{ label(row.key) }}</RouterLink>
              </TableCell>
              <TableCell class="text-right text-xs" :class="row.delta_pct !== null && signClass(-Number(row.delta_pct))">
                {{ delta(row.delta_pct) }}
              </TableCell>
              <TableCell
                v-for="(cell, index) in row.cells"
                :key="index"
                class="text-right"
                :class="!Number(cell) && 'text-muted-foreground'"
                :style="{ background: heat(block, cell) }"
              >
                <RouterLink v-if="Number(cell)" :to="ofCategory(row.key, months[index])" class="hover:underline">
                  <Money :amount="cell" :decimals="0" :currency="props.report.meta.currency" />
                </RouterLink>
                <template v-else>—</template>
              </TableCell>
              <TableCell class="text-right font-medium"><Money :amount="row.total" :decimals="0" :currency="props.report.meta.currency" /></TableCell>
              <TableCell class="text-right"><Money :amount="row.average" :decimals="0" :currency="props.report.meta.currency" /></TableCell>
              <TableCell class="text-right">{{ formatPct(row.share, { decimals: 0 }) }}</TableCell>
            </TableRow>
            <TableRow class="font-medium">
              <TableCell class="sticky left-0 z-[1] bg-card">{{ t('report.total') }}</TableCell>
              <TableCell />
              <TableCell
                v-for="(cell, index) in block.totals"
                :key="index"
                class="text-right"
                :class="!Number(cell) && 'text-muted-foreground'"
              >
                <Money v-if="Number(cell)" :amount="cell" :decimals="0" :currency="props.report.meta.currency" />
                <template v-else>—</template>
              </TableCell>
              <TableCell class="text-right"><Money :amount="block.total" :decimals="0" :currency="props.report.meta.currency" /></TableCell>
              <TableCell colspan="2" />
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </Card>
  </div>
</template>
