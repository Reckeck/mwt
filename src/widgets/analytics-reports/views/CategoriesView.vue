<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'

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
  <div class="grid gap-4">
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
            :class="Number(row.delta_pct) > 0 ? 'text-destructive' : 'text-emerald-600 dark:text-emerald-400'"
          >
            {{ Number(row.delta_pct) > 0 ? '+' : '' }}{{ row.delta_pct }}%
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
        <table class="w-full text-sm tabular-nums">
          <thead class="text-xs text-muted-foreground">
            <tr>
              <th class="py-1 pr-2 text-left font-normal">{{ t('report.categories.category') }}</th>
              <th v-for="month in props.report.meta.months" :key="month" class="px-1.5 py-1 text-right font-normal capitalize">
                {{ monthLabel(month, activeLocale) }}
              </th>
              <th class="px-2 py-1 text-right font-normal">{{ t('report.total') }}</th>
              <th class="px-2 py-1 text-right font-normal">{{ t('report.categories.average') }}</th>
              <th class="px-2 py-1 text-right font-normal">{{ t('report.categories.share') }}</th>
              <th class="py-1 pl-2 text-right font-normal">Δ</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in block.rows" :key="row.key" class="border-t border-border">
              <td class="max-w-48 truncate py-1.5 pr-2">{{ label(row.key) }}</td>
              <td
                v-for="(cell, index) in row.cells"
                :key="index"
                class="px-1.5 py-1.5 text-right"
                :style="{ background: heat(block, cell) }"
              >
                {{ Number(cell) ? money(cell) : '' }}
              </td>
              <td class="px-2 py-1.5 text-right font-medium">{{ money(row.total) }}</td>
              <td class="px-2 py-1.5 text-right">{{ money(row.average) }}</td>
              <td class="px-2 py-1.5 text-right">{{ row.share }}%</td>
              <td class="py-1.5 pl-2 text-right text-xs">{{ row.delta_pct === null ? '—' : `${row.delta_pct}%` }}</td>
            </tr>
            <tr class="border-t-2 border-border font-medium">
              <td class="py-1.5 pr-2">{{ t('report.total') }}</td>
              <td v-for="(cell, index) in block.totals" :key="index" class="px-1.5 py-1.5 text-right">{{ money(cell) }}</td>
              <td class="px-2 py-1.5 text-right">{{ money(block.total) }}</td>
              <td colspan="3" />
            </tr>
          </tbody>
        </table>
      </div>
    </Card>
    <p v-if="!props.report.groups.length" class="py-12 text-center text-sm text-muted-foreground">{{ t('report.empty') }}</p>
  </div>
</template>
