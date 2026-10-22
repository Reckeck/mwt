<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { BudgetDialog } from '@/features/budget-edit'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import type { SpendingCategory } from '../../model/use-spending'

/**
 * The period's categories (Money 1 · 1): the sum, the bar against the biggest,
 * the usual month, the limit and «Задать лимит». On a phone each is a card.
 */
const props = defineProps<{ rows: SpendingCategory[]; currency: string }>()

const { t } = useI18n()
const categories = useCategories()
const money = (value: string | null) => (value === null ? '—' : formatMoney(value, { decimals: 0, currency: props.currency }))

function limitOf(budget: Budget | null): string {
  if (!budget) return t('money.categories.noLimit')
  const value = formatMoney(budget.limit, { decimals: 0, currency: budget.currency })
  return budget.period === 'month' ? value : `${value} / ${t(`budget.periodKind.${budget.period}`)}`
}

/** What a row shows, worked out once per row. */
const view = computed(() => {
  const biggest = Math.max(1, ...props.rows.map((row) => Number(row.total)))
  return props.rows.map((row) => {
    const label = row.key ? categoryKeyLabel(row.key, categories.data.value, t) : t('money.categories.none')
    return {
      row,
      label,
      width: `${(Number(row.total) / biggest) * 100}%`,
      total: money(row.total),
      average: money(row.average),
      limit: limitOf(row.budget),
      action: row.budget ? t('money.categories.editLimit') : t('money.categories.setLimit'),
      phoneAction: row.budget ? t('money.categories.editLimit') : t('money.categories.setLimitOn', { category: label }),
    }
  })
})

const dialog = ref(false)
const editing = ref<{ budget: Budget | null; category: string }>({ budget: null, category: '' })
function edit(row: SpendingCategory) {
  editing.value = { budget: row.budget, category: row.key }
  dialog.value = true
}
</script>

<template>
  <section data-tour="spending-categories">
    <div class="overflow-hidden rounded-xl border bg-card shadow-sm max-md:hidden">
      <Table>
        <TableHeader>
          <TableRow class="text-xs">
            <TableHead class="px-3 text-muted-foreground">{{ t('money.categories.category') }}</TableHead>
            <TableHead class="w-2/5 px-3 text-muted-foreground">{{ t('money.categories.share') }}</TableHead>
            <TableHead class="px-3 text-right text-muted-foreground">{{ t('money.categories.sum') }}</TableHead>
            <TableHead class="px-3 text-right text-muted-foreground">{{ t('money.categories.average') }}</TableHead>
            <TableHead class="px-3 text-right text-muted-foreground">{{ t('money.categories.limit') }}</TableHead>
            <TableHead class="px-3"><span class="sr-only">{{ t('money.categories.actions') }}</span></TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="item in view" :key="item.row.key">
            <TableCell class="max-w-0 truncate px-3 font-medium" :title="item.label">{{ item.label }}</TableCell>
            <TableCell class="px-3">
              <div class="h-2 overflow-hidden rounded bg-muted"><div class="h-full rounded bg-primary" :style="{ width: item.width }" /></div>
            </TableCell>
            <TableCell class="px-3 text-right font-semibold tabular-nums">{{ item.total }}</TableCell>
            <TableCell class="px-3 text-right text-muted-foreground tabular-nums">{{ item.average }}</TableCell>
            <TableCell class="px-3 text-right text-muted-foreground tabular-nums">{{ item.limit }}</TableCell>
            <TableCell class="px-3 text-right">
              <Button variant="outline" size="sm" @click="edit(item.row)">{{ item.action }}</Button>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
    <div class="grid gap-3 md:hidden">
      <div v-for="item in view" :key="item.row.key" class="grid gap-2 rounded-xl border bg-card p-4 shadow-sm">
        <div class="flex items-baseline gap-2">
          <span class="min-w-0 flex-1 truncate font-medium">{{ item.label }}</span>
          <b class="whitespace-nowrap tabular-nums">{{ item.total }}</b>
        </div>
        <div class="h-2 overflow-hidden rounded bg-muted"><div class="h-full rounded bg-primary" :style="{ width: item.width }" /></div>
        <div class="flex justify-between gap-2 text-xs text-muted-foreground">
          <span>{{ t('money.categories.average') }} {{ item.average }}</span>
          <span>{{ item.limit }}</span>
        </div>
        <Button variant="outline" class="h-11" @click="edit(item.row)">{{ item.phoneAction }}</Button>
      </div>
    </div>
    <BudgetDialog v-model:open="dialog" :budget="editing.budget" :category="editing.category" />
  </section>
</template>
