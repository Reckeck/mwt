import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { useReport } from '@/entities/report'
import { call } from '@/shared/api/ipc'
import { shiftMonth } from '@/shared/lib/period'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

import { SPENT_GROUPS, monthFilter } from './use-spent-glance'

/** «Последние оплаты»: three, newest first. */
const LAST = 3

/** The category's row of a categories report, and the month's whole spending. */
function pick(
  data: AnalyticsReport | undefined,
  key: string,
): { total: string; average: string | null; spent: number } {
  const groups = data && 'categories' in data ? data.categories.groups : []
  const rows = groups
    .filter((block) => block.group && SPENT_GROUPS.includes(block.group))
    .flatMap((block) => block.rows)
  const row = groups.flatMap((block) => block.rows).find((one) => one.key === key)
  const spent = rows.reduce((sum, one) => sum + Math.max(0, Number(one.total)), 0)
  return { total: row?.total ?? '0', average: row?.average ?? null, spent }
}

/**
 * A category in Ctrl K (Paths 2 · 8): the month against the one before, its
 * share of the month, the limit and the last three payments.
 */
export function useCategoryGlance(
  category: MaybeRefOrGetter<string>,
  month: MaybeRefOrGetter<string>,
  enabled: MaybeRefOrGetter<boolean>,
) {
  const on = { enabled: () => toValue(enabled) && Boolean(toValue(category)) }
  const now = useReport('categories', () => monthFilter(toValue(month)), on)
  const before = useReport('categories', () => monthFilter(shiftMonth(toValue(month), -1)), on)
  const statuses = useQuery({
    queryKey: ['budget-status'],
    ...on,
    queryFn: () => call<BudgetStatus[]>('get_budget_status'),
  })
  const payments = useQuery({
    // no placeholder: another category's payments
    queryKey: computed(() => ['transactions', 'glance-category', toValue(category)]),
    ...on,
    queryFn: () =>
      call<TxPage>('list_transactions_page', {
        filter: emptyTxFilter({ kinds: ['payment'], category: toValue(category) }),
        offset: 0,
        limit: LAST,
      }),
  })

  const current = computed(() => pick(now.data.value, toValue(category)))
  const limit = computed(
    () =>
      statuses.data.value?.find((status) => {
        const target = status.budget.target
        return (
          status.budget.active &&
          typeof target === 'object' &&
          'category' in target &&
          target.category === toValue(category)
        )
      }) ?? null,
  )

  return {
    total: computed(() => current.value.total),
    totalBefore: computed(() => pick(before.data.value, toValue(category)).total),
    average: computed(() => current.value.average),
    /** Of the month's spending, 0–100. */
    pct: computed(() =>
      current.value.spent ? Math.round((Number(current.value.total) / current.value.spent) * 100) : 0,
    ),
    currency: computed(() => {
      const data = now.data.value
      return data && 'categories' in data ? data.categories.meta.currency : ''
    }),
    limit,
    payments: computed(() => payments.data.value?.items ?? []),
  }
}
