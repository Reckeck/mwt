<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { MoneyPhrase } from '@/entities/currency'
import { BudgetDialog, type BudgetSpending } from '@/features/budget-edit'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMonthKey, pluralForm } from '@/shared/lib/format'
import { composeSums, type Sum, type SumPart } from '@/shared/lib/money-parts'
import { Button } from '@/shared/ui/button'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { comparable, limitBar } from '../../lib/limit-view'
import type { SpendingCategory } from '../../model/use-spending'
import { CategorySheet } from '../category-sheet'

/**
 * The period's categories (Money 1 · 1, 2, 5): the sum, the bar (against the
 * biggest, or against the limit), the usual month, the limit. A row opens the
 * category; `Set a limit` opens the limit wizard. On a phone each is a card.
 */
const props = defineProps<{ rows: SpendingCategory[]; currency: string; filter: ReportFilter; months: string[] }>()

const { t, locale } = useI18n()
const categories = useCategories()
const from = computed(() => props.filter.from ?? '')
const to = computed(() => props.filter.to ?? '')
const sum = (value: string, currency = props.currency, bare = false): Sum => ({ amount: value, currency, decimals: 0, bare })
const money = (value: string | null): SumPart[] => (value === null ? [{ text: '—' }] : [sum(value)])
const words = (text: string): SumPart[] => [{ text }]

function limitOf(row: SpendingCategory): { text: SumPart[]; hint: SumPart[] | null } {
  const budget = row.budget
  if (!budget) return { text: words(t('money.categories.noLimit')), hint: null }
  const { mark, parts } = composeSums()
  if (!comparable(budget, from.value, to.value, props.currency)) {
    const value = sum(budget.limit, budget.currency)
    return { text: budget.period === 'month' ? [value] : [value, { text: ` / ${t(`budget.periodKind.${budget.period}`)}` }], hint: null }
  }
  const rest = Number(budget.limit) - Number(row.total)
  return {
    text: parts(t('money.categories.ofLimit', { spent: mark(sum(row.total, props.currency, true)), limit: mark(sum(budget.limit)) })),
    hint: parts(
      rest >= 0
        ? t('money.categories.left', { left: mark(sum(String(rest))) })
        : t('money.categories.over', { over: mark(sum(String(-rest))) }),
    ),
  }
}

/** The limit's words for the category sheet: `Limit…`. */
function sheetLimit(item: (typeof view.value)[number] | null): SumPart[] {
  if (!item?.row.budget) return words(t('money.category.noLimit'))
  const { mark, parts } = composeSums()
  return parts(t('money.category.limit', { limit: mark(item.limit.text) }))
}

/** What a row shows, worked out once per row. */
const view = computed(() => {
  const biggest = Math.max(1, ...props.rows.map((row) => Number(row.total)))
  return props.rows.map((row) => {
    const label = row.key ? categoryKeyLabel(row.key, categories.data.value, t) : t('money.categories.none')
    const bar = row.budget && comparable(row.budget, from.value, to.value, props.currency)
      ? limitBar(row.total, row.budget.limit, to.value)
      : { percent: (Number(row.total) / biggest) * 100, tone: 'primary' as const }
    const unpriced = row.unpriced ? t(`money.categories.unpriced.${pluralForm(row.unpriced, locale.value)}`, { count: row.unpriced }) : ''
    return { row, label, bar, total: money(row.total), average: money(row.average), limit: limitOf(row), unpriced }
  })
})

/** The period as the category's header names it, and as the limit's hints do. */
const period = computed(() =>
  from.value === to.value
    ? formatMonthKey(to.value, { month: 'long', year: 'numeric' }).replace(/\s*г\.$/, '')
    : `${formatMonthKey(from.value, { month: 'short', year: 'numeric' })} — ${formatMonthKey(to.value, { month: 'short', year: 'numeric' })}`,
)
const when = computed(() => (from.value === to.value ? t(`period.in.${to.value.slice(5, 7)}`) : period.value))

const opened = ref<(typeof view.value)[number] | null>(null)
const sheet = ref(false)
function show(item: (typeof view.value)[number]) {
  opened.value = item
  sheet.value = true
}

/** `?category=` — `Open in Spending` from Ctrl K (Paths 2 · 8) opens that category once its row is in. */
const route = useRoute()
const router = useRouter()
watch(
  [() => route.query.category, view],
  ([asked, items]) => {
    if (typeof asked !== 'string') return
    const item = items.find((one) => one.row.key === asked)
    if (!item) return
    show(item)
    const { category: _category, ...rest } = route.query
    void router.replace({ query: rest })
  },
  { immediate: true },
)

const dialog = ref(false)
const editing = ref<{ budget: Budget | null; category: string; spending: BudgetSpending | null }>({ budget: null, category: '', spending: null })
function edit(row: SpendingCategory) {
  const spending = { spent: row.total, average: row.average, currency: props.currency, when: when.value }
  editing.value = { budget: row.budget, category: row.key, spending }
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
            <TableHead class="w-36 px-3"><span class="sr-only">{{ t('money.categories.actions') }}</span></TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="item in view" :key="item.row.key" class="cursor-pointer" @click="show(item)">
            <TableCell class="max-w-0 px-3">
              <Button
                variant="ghost"
                size="block"
                class="block w-full truncate p-0 text-left font-medium hover:bg-transparent"
                :title="item.label"
                data-category-row
              >
                {{ item.label }}
              </Button>
            </TableCell>
            <TableCell class="px-3"><ProgressBar :value="item.bar.percent" :tone="item.bar.tone" /></TableCell>
            <TableCell class="px-3 text-right font-semibold tabular-nums">
              <MoneyPhrase :parts="item.total" />
              <span v-if="item.unpriced" class="block text-xs font-normal text-muted-foreground">{{ item.unpriced }}</span>
            </TableCell>
            <TableCell class="px-3 text-right text-muted-foreground tabular-nums"><MoneyPhrase :parts="item.average" /></TableCell>
            <TableCell class="px-3 text-right tabular-nums" :class="!item.row.budget && 'text-muted-foreground'">
              <MoneyPhrase :parts="item.limit.text" />
              <span v-if="item.limit.hint" class="block text-xs text-muted-foreground"><MoneyPhrase :parts="item.limit.hint" /></span>
            </TableCell>
            <TableCell class="px-3 text-right">
              <Button v-if="!item.row.budget" variant="outline" size="sm" data-category-limit @click.stop="edit(item.row)">
                {{ t('money.categories.setLimit') }}
              </Button>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
    <div class="grid gap-3 md:hidden">
      <div v-for="item in view" :key="item.row.key" class="grid gap-2 rounded-xl border bg-card p-4 shadow-sm">
        <Button variant="ghost" size="block" class="flex items-baseline gap-2 p-0 text-left hover:bg-transparent" data-category-row @click="show(item)">
          <span class="min-w-0 flex-1 truncate font-medium">{{ item.label }}</span>
          <b class="whitespace-nowrap tabular-nums"><MoneyPhrase :parts="item.total" /></b>
          <span v-if="item.unpriced" class="text-xs whitespace-nowrap text-muted-foreground">{{ item.unpriced }}</span>
        </Button>
        <ProgressBar :value="item.bar.percent" :tone="item.bar.tone" />
        <div class="flex justify-between gap-2 text-xs text-muted-foreground">
          <span>{{ t('money.categories.average') }} <MoneyPhrase :parts="item.average" /></span>
          <span>
            <MoneyPhrase :parts="item.limit.text" />
            <template v-if="item.limit.hint"> · <MoneyPhrase :parts="item.limit.hint" /></template>
          </span>
        </div>
        <Button v-if="!item.row.budget" variant="outline" class="h-11" data-category-limit @click="edit(item.row)">
          {{ t('money.categories.setLimitOn', { category: item.label }) }}
        </Button>
      </div>
    </div>
    <CategorySheet
      v-model:open="sheet"
      :row="opened?.row ?? null"
      :label="opened?.label ?? ''"
      :period="period"
      :filter="props.filter"
      :months="props.months"
      :currency="props.currency"
      :limit="sheetLimit(opened)"
      @limit="sheet = false; opened && edit(opened.row)"
    />
    <BudgetDialog v-model:open="dialog" :budget="editing.budget" :category="editing.category" :spending="editing.spending" />
  </section>
</template>
