<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { useQuery } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'

import { BUDGET_WARN_PERCENT, BudgetDeleteDialog, BudgetDialog, type BudgetSpending } from '@/features/budget-edit'
import { useReport } from '@/widgets/analytics-reports'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { pluralForm } from '@/shared/lib/format'
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
const editing = ref<Budget | null>(null)
const deleting = ref<{ budget: Budget; name: string } | null>(null)
const deleteOpen = ref(false)
function create(key = '') {
  editing.value = null
  category.value = key
  open.value = true
}
function edit(budget: Budget) {
  editing.value = budget
  category.value = ''
  open.value = true
}
function askDelete(budget: Budget, name: string) {
  deleting.value = { budget, name }
  deleteOpen.value = true
}

/** What each category spent in the period and its usual month: the hint under an edited limit's sum. */
const spendingOf = computed(() => {
  const data = report.data.value && 'categories' in report.data.value ? report.data.value.categories : null
  const month = (props.filter.from ?? '').slice(5, 7)
  const when = month ? t(`period.in.${month}`) : ''
  const capital = when.charAt(0).toUpperCase() + when.slice(1)
  const rows = new Map((data?.groups ?? []).flatMap((block) => block.rows).map((row) => [row.key, row]))
  return (budget: Budget): BudgetSpending | null => {
    const row = typeof budget.target === 'object' && 'category' in budget.target ? rows.get(budget.target.category) : undefined
    return data && row ? { spent: row.total, average: row.average ?? null, currency: data.meta.currency, when: capital } : null
  }
})

/** `?new=limit&category=…` from a glance or Ctrl K («Задать лимит на „Продукты“»). */
const route = useRoute()
const router = useRouter()
watch(
  () => route.query.new,
  (asked) => {
    if (asked !== 'limit') return
    create(typeof route.query.category === 'string' ? route.query.category : '')
    const { new: _new, category: _category, ...rest } = route.query
    void router.replace({ query: rest })
  },
  { immediate: true },
)
</script>

<template>
  <section class="grid content-start gap-3 rounded-xl border bg-card p-4 shadow-sm" data-tour="plan-limits">
    <header class="flex items-center justify-between gap-2">
      <h2 class="heading-section">{{ t('money.plan.limits') }}</h2>
      <Button variant="outline" size="sm" data-plan-limit @click="create()"><Plus />{{ t('money.plan.limit') }}</Button>
    </header>
    <ul v-if="rows.length" class="grid gap-3">
      <li v-for="row in rows" :key="row.status.budget.id" class="grid gap-1">
        <div class="flex items-center justify-between gap-2 text-sm">
          <span class="min-w-0 flex-1 truncate">{{ row.label }}</span>
          <span class="shrink-0 tabular-nums">
            <!-- The limit's mark names the currency for both, so the spent sum shows the number only. -->
            <Money bare :amount="row.status.spent" :currency="row.status.budget.currency" :decimals="0" />
            /
            <Money :amount="row.status.budget.limit" :currency="row.status.budget.currency" :decimals="0" />
          </span>
          <Button
            variant="outline"
            size="icon-sm"
            class="shrink-0 max-md:size-10"
            :title="t('budget.edit')"
            :aria-label="t('budget.edit')"
            data-plan-limit-edit
            @click="edit(row.status.budget)"
          >
            <Pencil class="size-4" />
          </Button>
          <Button
            variant="outline"
            size="icon-sm"
            class="shrink-0 text-destructive max-md:size-10"
            :title="t('budget.delete')"
            :aria-label="t('budget.delete')"
            data-plan-limit-delete
            @click="askDelete(row.status.budget, row.label)"
          >
            <Trash2 class="size-4" />
          </Button>
        </div>
        <ProgressBar :value="row.used" :tone="row.tone" size="sm" />
        <p class="text-xs text-muted-foreground max-md:hidden">
          <template v-if="Number(row.status.remaining) > 0">
            <i18n-t keypath="money.plan.leftFor">
              <template #amount><Money :amount="row.status.remaining" :currency="row.status.budget.currency" :decimals="0" /></template>
              <template #days>{{ row.days }}</template>
            </i18n-t>
            {{ t(`money.plan.days.${pluralForm(row.days, locale)}`) }}
          </template>
          <i18n-t v-else keypath="money.plan.over">
            <template #amount>
              <Money :amount="String(-Number(row.status.remaining))" :currency="row.status.budget.currency" :decimals="0" />
            </template>
          </i18n-t>
        </p>
      </li>
    </ul>
    <div v-else-if="empty" class="grid justify-items-start gap-2 text-sm">
      <p>{{ t('money.plan.noLimits') }}</p>
      <p v-if="top" class="text-muted-foreground">
        <i18n-t keypath="money.plan.noLimitsHint">
          <template #when>{{ top.spending.when }}</template>
          <template #name>{{ topName }}</template>
          <template #amount><Money :amount="top.spending.spent" :currency="top.spending.currency" :decimals="0" /></template>
        </i18n-t>
      </p>
      <Button v-if="top" size="sm" class="max-md:h-10" data-plan-limit-top @click="create(top.key)">{{ t('money.plan.limitOn', { name: topName }) }}</Button>
    </div>
    <BudgetDialog
      v-model:open="open"
      :budget="editing"
      :category="category"
      :spending="editing ? spendingOf(editing) : category && top?.key === category ? top.spending : null"
    />
    <BudgetDeleteDialog v-model:open="deleteOpen" :budget="deleting?.budget ?? null" :name="deleting?.name ?? ''" />
  </section>
</template>
