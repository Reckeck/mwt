import { computed } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { useMonthBalance } from '@/entities/report'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { currentMonth } from '@/shared/lib/period'

/**
 * What the top of the Home screen reads (Home 1): the capital and the things to do,
 * the day's allowance, this month's spending and its overall limit. All sums come
 * from the backend; nothing is added up here.
 */
export function useHomeToday() {
  const today = useQuery({
    queryKey: ['dashboard-today', 'month'],
    queryFn: () => call<DashboardToday>('get_dashboard_today', { period: 'month' }),
    placeholderData: keepPreviousData,
  })
  /** The capital itself: the same figure as the header, so Home never shows a second one. */
  const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
  const daily = useQuery({ queryKey: ['daily-budget'], queryFn: () => call<DailyBudget>('get_daily_budget') })
  const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
  const month = currentMonth()
  const balance = useMonthBalance({
    scope: { store_ids: [], include_children: true },
    from: month,
    to: month,
    year: Number(month.slice(0, 4)),
    currency: null,
    real_terms: false,
  })
  const tree = useStoreTree()

  /** The limit on all spending this month, if there is one. */
  const limit = computed(
    () =>
      statuses.data.value?.find(
        (status) => status.budget.active && status.budget.target === 'total' && status.budget.period === 'month',
      ) ?? null,
  )
  /** A fresh vault: no wallet yet, so every figure is a hint instead. */
  const fresh = computed(() => tree.data.value !== undefined && tree.data.value.length === 0)

  const loading = computed(() => [today, summary, daily, balance].some((query) => query.isPending.value))
  const error = computed(() => today.error.value ?? daily.error.value ?? balance.error.value ?? null)
  function refetch() {
    for (const query of [today, summary, daily, balance, statuses]) void query.refetch()
  }
  return { today, summary, daily, balance, limit, fresh, month, loading, error, refetch }
}
