import { computed } from 'vue'

import { currentMonth } from '@/shared/lib/period'

import { useReport } from './use-report'

/**
 * This month's total per category, from the same report «Money › Spending»
 * draws its table from (every wallet, the budget currency). Zero sums are left
 * out: a category without spending has no entry.
 */
export function useMonthCategoryTotals() {
  const month = currentMonth()
  const filter: ReportFilter = {
    scope: { store_ids: [], include_children: true },
    from: month,
    to: month,
    year: null,
    currency: null,
    real_terms: false,
  }
  const report = useReport('categories', filter)
  const totals = computed(() => {
    const data = report.data.value
    const map = new Map<string, string>()
    if (!data || !('categories' in data)) return map
    for (const block of data.categories.groups) {
      for (const row of block.rows) if (row.key && Number(row.total) > 0) map.set(row.key, row.total)
    }
    return map
  })
  const currency = computed(() => {
    const data = report.data.value
    return data && 'categories' in data ? data.categories.meta.currency : ''
  })
  return { month, totals, currency }
}
