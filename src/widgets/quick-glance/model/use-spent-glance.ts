import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { useMonthBalance, useReport } from '@/entities/report'
import { call } from '@/shared/api/ipc'
import { lastDay } from '@/shared/lib/drill'
import { shiftMonth } from '@/shared/lib/period'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

/** Spending groups, as the «Траты» tab counts them. */
export const SPENT_GROUPS: CategoryGroup[] = ['fixed', 'daily']
/** «Последние оплаты»: three, newest first. */
const LAST = 3

export const monthFilter = (month: string): ReportFilter => ({
  scope: { store_ids: [], include_children: true },
  from: month,
  to: month,
  year: Number(month.slice(0, 4)),
  currency: null,
  real_terms: false,
})

export interface SpentShare {
  key: string
  total: string
  /** Of the month's spending, 0–100. */
  pct: number
}

/**
 * What «Потрачено в октябре» opens (Paths 1 · 2): the month against the one
 * before, the categories' shares and the last payments. The queries are the
 * ones «Траты» and Home already run, so a glance mostly reads the cache.
 */
export function useSpentGlance(month: MaybeRefOrGetter<string>, enabled: MaybeRefOrGetter<boolean>) {
  const filter = computed(() => monthFilter(toValue(month)))
  const previous = computed(() => monthFilter(shiftMonth(toValue(month), -1)))
  const on = { enabled: () => toValue(enabled) }
  const balance = useMonthBalance(filter, on)
  const before = useMonthBalance(previous, on)
  const report = useReport('categories', filter, on)
  const statuses = useQuery({
    queryKey: ['budget-status'],
    ...on,
    queryFn: () => call<BudgetStatus[]>('get_budget_status'),
  })
  const payments = useQuery({
    // no placeholder: another month's payments
    queryKey: computed(() => ['transactions', 'glance-payments', toValue(month)]),
    ...on,
    queryFn: () =>
      call<TxPage>('list_transactions_page', {
        filter: emptyTxFilter({ kinds: ['payment'], from: `${toValue(month)}-01`, to: lastDay(toValue(month)) }),
        offset: 0,
        limit: LAST,
      }),
  })

  const shares = computed<SpentShare[]>(() => {
    const data = report.data.value
    const rows =
      data && 'categories' in data
        ? data.categories.groups
            .filter((block) => block.group && SPENT_GROUPS.includes(block.group))
            .flatMap((block) => block.rows)
        : []
    const spent = rows
      .filter((row) => Number(row.total) > 0)
      .sort((left, right) => Number(right.total) - Number(left.total))
    const sum = spent.reduce((total, row) => total + Number(row.total), 0) || 1
    return spent.map((row) => ({ key: row.key, total: row.total, pct: Math.round((Number(row.total) / sum) * 100) }))
  })
  /** The month's limit on all spending, if any. */
  const limit = computed(
    () =>
      statuses.data.value?.find(
        (status) => status.budget.active && status.budget.target === 'total' && status.budget.period === 'month',
      ) ?? null,
  )

  return {
    spent: computed(() => balance.data.value?.total.spent ?? '0'),
    spentBefore: computed(() => before.data.value?.total.spent ?? '0'),
    currency: computed(() => balance.data.value?.currency ?? ''),
    shares,
    limit,
    payments: computed(() => payments.data.value?.items ?? []),
    loading: computed(() => balance.isPending.value || report.isPending.value),
  }
}
