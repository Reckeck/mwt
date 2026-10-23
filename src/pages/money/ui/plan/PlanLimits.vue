<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { BUDGET_WARN_PERCENT, BudgetDialog, type BudgetSpending } from '@/features/budget-edit'
import { useReport } from '@/widgets/analytics-reports'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney, pluralForm } from '@/shared/lib/format'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { Button } from '@/shared/ui/button'
import { ProgressBar, type ProgressTone } from '@/shared/ui/progress-bar'

/**
 * The plan's limits (Money 2 · 9, 10): each one's spent of the limit and what
 * is left for the days ahead. Without any, the period's biggest category is
 * offered first.
 */
const props = defineProps<{ filter: ReportFilter }>()

const { t, locale } = useI18n()
const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })
const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
const categories = useCategories()
const tree = useStoreTree()
const report = useReport('categories', () => props.filter)

function targetText(target: BudgetTarget): string {
  if (target === 'total') return t('budget.targetKind.total')
  if ('category' in target) return categoryKeyLabel(target.category, categories.data.value, t)
  const id = target.store.store_ids[0] ?? ''
  return flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === id)?.store.name ?? id
}

const DAY = 86_400_000
const rows = computed(() =>
  (statuses.data.value ?? [])
    .filter((status) => status.budget.active)
    .map((status) => {
      const used = Math.round(Number(status.used) * 100)
      const tone: ProgressTone = used >= 100 ? 'destructive' : status.budget.warn && used >= BUDGET_WARN_PERCENT ? 'warning' : 'primary'
      const days = Math.max(1, Math.ceil((Date.parse(status.period_end) - Date.now()) / DAY))
      return { status, used, tone, days, label: targetText(status.budget.target) }
    }),
)
const money = (value: string, currency: string) => formatMoney(value, { decimals: 0, currency })
const bare = (value: string, currency: string) => money(value, currency).replace(/\s*\D+$/, '')

/** The period's biggest category: what a first limit is best set on. */
const top = computed<{ key: string; spending: BudgetSpending } | null>(() => {
  const data = report.data.value && 'categories' in report.data.value ? report.data.value.categories : null
  const row = data?.groups
    .filter((block) => block.group === 'fixed' || block.group === 'daily')
    .flatMap((block) => block.rows)
   .sort((left, right) => Number(right.total) - Number(left.total))[0]
  if (!data || !row || !(Number(row.total) > 0)) return null
  const month = (props.filter.from ?? '').slice(5, 7)
  const when = month ? t(`period.in.${month}`) : ''
  const capital = when.charAt(0).toUpperCase() + when.slice(1)
  return { key: row.key, spending: { spent: row.total, average: row.average ?? null, currency: data.meta.currency, when: capital } }
})
const topName = computed(() => (top.value ? categoryKeyLabel(top.value.key, categories.data.value, t) : ''))
const empty = computed(() => budgets.isSuccess.value && !rows.value.length)

const open = ref(false)
const category = ref('')
function create(key = '') {
  category.value = key
  open.value = true
}
</script>

<template>
  <section class="grid content-start gap-3 rounded-xl border bg-card p-4 shadow-sm" data-tour="plan-limits">
    <header class="flex items-center justify-between gap-2">
      <h2 class="heading-section">{{ t('money.plan.limits') }}</h2>
      <Button variant="outline" size="sm" data-plan-limit @click="create()"><Plus />{{ t('money.plan.limit') }}</Button>
    </header>
    <ul v-if="rows.length" class="grid gap-3">
      <li v-for="row in rows" :key="row.status.budget.id" class="grid gap-1">
        <div class="flex items-baseline justify-between gap-2 text-sm">
          <span class="min-w-0 truncate">{{ row.label }}</span>
          <span class="shrink-0 tabular-nums">
            {{ bare(row.status.spent, row.status.budget.currency) }} / {{ money(row.status.budget.limit, row.status.budget.currency) }}
          </span>
        </div>
        <ProgressBar :value="row.used" :tone="row.tone" size="sm" />
        <p class="text-xs text-muted-foreground max-md:hidden">
          <template v-if="Number(row.status.remaining) > 0">
            {{ t('money.plan.leftFor', { amount: money(row.status.remaining, row.status.budget.currency), days: row.days }) }}
            {{ t(`money.plan.days.${pluralForm(row.days, locale)}`) }}
          </template>
          <template v-else>{{ t('money.plan.over', { amount: money(String(-Number(row.status.remaining)), row.status.budget.currency) }) }}</template>
        </p>
      </li>
    </ul>
    <div v-else-if="empty" class="grid justify-items-start gap-2 text-sm">
      <p>{{ t('money.plan.noLimits') }}</p>
      <p v-if="top" class="text-muted-foreground">
        {{ t('money.plan.noLimitsHint', { when: top.spending.when, name: topName, amount: money(top.spending.spent, top.spending.currency) }) }}
      </p>
      <Button v-if="top" size="sm" class="max-md:h-10" data-plan-limit-top @click="create(top.key)">{{ t('money.plan.limitOn', { name: topName }) }}</Button>
    </div>
    <BudgetDialog v-model:open="open" :category="category" :spending="category && top?.key === category ? top.spending : null" />
  </section>
</template>
