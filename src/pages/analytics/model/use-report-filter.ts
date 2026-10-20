import { type Ref, computed, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { type AnalyticsGroup, MONTH_GROUPS, REAL_TERMS_GROUPS } from './analytics-groups'

const KEYS = ['store', 'from', 'to', 'year', 'cur', 'real', 'month'] as const
type Key = (typeof KEYS)[number]

const empty = (): ReportFilter => ({
  scope: { store_ids: [], include_children: true },
  from: null,
  to: null,
  year: null,
  currency: null,
  real_terms: false,
})

/**
 * One filter for every report tab, kept in the address (`?from=2026-01&store=a,b`)
 * so switching tabs, Back and a reload keep the period and wallets.
 *
 * The month of the overview and the balance has its own key (`month`): picking it must not move the
 * period of the other sections. «In today's money» applies to the flow and
 * wealth sections only; elsewhere the figures stay nominal.
 */
export function useReportFilter(group: Ref<AnalyticsGroup>) {
  const route = useRoute()
  const router = useRouter()
  const one = (key: string) => (typeof route.query[key] === 'string' ? (route.query[key] as string) : null)

  const filter = reactive<ReportFilter>(empty())
  const stores = one('store')
  if (stores) filter.scope = { store_ids: stores.split(',').filter(Boolean), include_children: true }
  filter.from = one('from')
  filter.to = one('to')
  filter.year = Number(one('year')) || null
  filter.currency = one('cur')
  filter.real_terms = one('real') === '1'
  const month = ref(one('month'))

  // One writer of the address, so two changes in a row cannot overwrite each other.
  watch(
    [filter, month],
    ([value, overviewMonth]) => {
      const query: Record<string, unknown> = { ...route.query }
      for (const key of KEYS) delete query[key]
      const next: Record<string, string | null> = {
        month: overviewMonth,
        store: value.scope.store_ids.join(',') || null,
        from: value.from,
        to: value.to,
        year: value.year ? String(value.year) : null,
        cur: value.currency,
        real: value.real_terms ? '1' : null,
      }
      for (const [key, entry] of Object.entries(next)) if (entry) query[key] = entry
      void router.replace({ query: query as Record<string, string> })
    },
    { deep: true },
  )

  const realTerms = computed(() => REAL_TERMS_GROUPS.includes(group.value))
  /** What the section and its export get: the overview's own month, today's money only where it applies. */
  const effective = computed<ReportFilter>(() => {
    const base = { ...filter, real_terms: filter.real_terms && realTerms.value }
    return MONTH_GROUPS.includes(group.value) ? { ...base, from: null, to: month.value } : base
  })

  /** Keys that change what this section shows. */
  const applies = (key: Key) => {
    if (key === 'real') return realTerms.value
    const byMonth = MONTH_GROUPS.includes(group.value)
    if (key === 'month') return byMonth
    if (key === 'from' || key === 'to') return !byMonth
    // The running balance counts every wallet in the budget currency.
    if (group.value === 'saldo') return false
    return true
  }
  /** How many fields narrow this section: «Сбросить (2)» shows when any. */
  const narrowed = computed(() => KEYS.filter((key) => applies(key) && one(key)).length)
  function reset() {
    Object.assign(filter, empty())
    month.value = null
  }

  return { filter, effective, month, realTerms, narrowed, reset }
}
