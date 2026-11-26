import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { flattenStoreTree } from '@/shared/lib/store-tree'

/** The limits with their status: each budget matched once to its spent figure, and a readable name for its target. */
export function useBudgetRows() {
  const { t } = useI18n()
  const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })
  const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
  const categories = useCategories()
  const tree = useStoreTree()

  function targetText(target: BudgetTarget): string {
    if (target === 'total') return t('budget.targetKind.total')
    if ('category' in target) return categoryKeyLabel(target.category, categories.data.value, t)
    const id = target.store.store_ids[0] ?? ''
    return (
      flattenStoreTree(tree.data.value ?? [])
        .find((node) => node.store.id === id)
        ?.path.join(' › ') ?? id
    )
  }

  const rows = computed(() => {
    const byId = new Map((statuses.data.value ?? []).map((status) => [status.budget.id, status]))
    return (budgets.data.value ?? []).map((budget) => ({
      budget,
      status: byId.get(budget.id) ?? null,
      name: targetText(budget.target),
    }))
  })
  const ready = computed(() => budgets.isSuccess.value && statuses.isSuccess.value)
  const loading = computed(() => budgets.isPending.value || statuses.isPending.value)
  const error = computed(() => budgets.error.value ?? statuses.error.value)
  const empty = computed(() => budgets.isSuccess.value && !budgets.data.value?.length)
  const refetch = () => Promise.all([budgets.refetch(), statuses.refetch()])

  return { rows, ready, loading, error, empty, refetch }
}
