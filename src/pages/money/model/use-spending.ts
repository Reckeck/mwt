import { type Ref, computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { useMonthBalance } from '@/entities/report'
import { call } from '@/shared/api/ipc'
import { shiftMonth } from '@/shared/lib/period'
import { useReport } from '@/widgets/analytics-reports'

/** Spending groups the tab lists: bills and everyday purchases. */
const SPENT: CategoryGroup[] = ['fixed', 'daily']

export interface SpendingCategory {
  key: string
  total: string
  /** The monthly average over the twelve months that end with the period. */
  average: string | null
  /** The category's limit, if it has one. */
  budget: Budget | null
}

/**
 * Everything the «Spending» tab shows for a period: the categories of the
 * period, the twelve months that end with it (bars and averages), the period's
 * balance. All sums come from the backend.
 */
export function useSpending(filter: Ref<ReportFilter>) {
  const to = computed(() => filter.value.to ?? '')
  /** Twelve months up to the period's end: the bars and the averages. */
  const year = computed<ReportFilter>(() => ({ ...filter.value, from: shiftMonth(to.value, -11), to: to.value }))

  const categories = useReport('categories', filter)
  const yearly = useReport('categories', year)
  const balance = useMonthBalance(filter)
  const months = useMonthBalance(year)
  const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })

  const report = (data: AnalyticsReport | undefined) => (data && 'categories' in data ? data.categories : null)
  const rowsOf = (data: AnalyticsReport | undefined) =>
    report(data)
      ?.groups.filter((block) => block.group && SPENT.includes(block.group))
      .flatMap((block) => block.rows) ?? []

  const rows = computed<SpendingCategory[]>(() => {
    const averages = new Map(rowsOf(yearly.data.value).map((row) => [row.key, row.average]))
    const limits = new Map(
      (budgets.data.value ?? [])
        .filter((budget) => budget.active && typeof budget.target === 'object' && 'category' in budget.target)
        .map((budget) => [(budget.target as { category: string }).category, budget]),
    )
    return rowsOf(categories.data.value)
      .filter((row) => Number(row.total) > 0)
      .sort((left, right) => Number(right.total) - Number(left.total))
      .map((row) => ({
        key: row.key,
        total: row.total,
        average: averages.get(row.key) ?? null,
        budget: limits.get(row.key) ?? null,
      }))
  })

  const currency = computed(() => balance.data.value?.currency ?? report(categories.data.value)?.meta.currency ?? '')
  const loading = computed(() => [categories, yearly, balance, months].some((query) => query.isPending.value))
  const error = computed(() => categories.error.value ?? balance.error.value)
  function refetch() {
    void categories.refetch()
    void balance.refetch()
    void months.refetch()
    void yearly.refetch()
  }

  return { rows, balance, months, currency, loading, error, refetch }
}
