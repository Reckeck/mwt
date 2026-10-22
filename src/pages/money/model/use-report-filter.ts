import { type Ref, computed, reactive, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { type MonthRange, presetRange } from '@/shared/lib/period'

import {
  ALL_WALLETS_GROUPS,
  GROUP_DEFAULT,
  type MoneyGroup,
  REAL_TERMS_GROUPS,
  UNFILTERED_GROUPS,
} from './money-groups'

const KEYS = ['store', 'from', 'to', 'cur', 'real'] as const
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
 * One filter for every tab of «Money», kept in the address (`?from=2026-01&store=a,b`)
 * so switching tabs, Back and a reload keep the period and wallets.
 *
 * An unset period is the tab's own default (`GROUP_DEFAULT`): the month for
 * spending, a year for where the money went. «In today's money» applies to the
 * flow and wealth tabs only; elsewhere the figures stay nominal.
 */
export function useReportFilter(group: Ref<MoneyGroup>) {
  const route = useRoute()
  const router = useRouter()
  const one = (key: string) => (typeof route.query[key] === 'string' ? (route.query[key] as string) : null)

  const filter = reactive<ReportFilter>(empty())
  /** The address is the source: a link to `/money` without a period resets it. */
  function read() {
    const stores = one('store') ?? ''
    if (stores !== filter.scope.store_ids.join(',')) {
      filter.scope = { store_ids: stores.split(',').filter(Boolean), include_children: true }
    }
    filter.from = one('from')
    filter.to = one('to')
    filter.currency = one('cur')
    filter.real_terms = one('real') === '1'
  }
  read()
  watch(() => route.query, read)

  // One writer of the address, so two changes in a row cannot overwrite each other.
  watch(
    filter,
    (value) => {
      const query: Record<string, unknown> = { ...route.query }
      for (const key of [...KEYS, 'month', 'year']) delete query[key]
      const next: Record<string, string | null> = {
        store: value.scope.store_ids.join(',') || null,
        from: value.from,
        to: value.to,
        cur: value.currency,
        real: value.real_terms ? '1' : null,
      }
      for (const [key, entry] of Object.entries(next)) if (entry) query[key] = entry
      void router.replace({ query: query as Record<string, string> })
    },
    { deep: true },
  )

  const realTerms = computed(() => REAL_TERMS_GROUPS.includes(group.value))
  const fallback = computed<MonthRange>(() => presetRange(GROUP_DEFAULT[group.value] ?? 'last12'))
  /** The period the picker shows; picking the tab's default clears the address. */
  const range = computed<MonthRange>({
    get: () => ({ from: filter.from ?? fallback.value.from, to: filter.to ?? fallback.value.to }),
    set: (value) => {
      const same = value.from === fallback.value.from && value.to === fallback.value.to
      filter.from = same ? null : value.from
      filter.to = same ? null : value.to
    },
  })
  /** What the tab and its export get: a set period, today's money only where it applies. */
  const effective = computed<ReportFilter>(() => ({
    ...filter,
    ...range.value,
    year: Number(range.value.to.slice(0, 4)),
    real_terms: filter.real_terms && realTerms.value,
    scope: ALL_WALLETS_GROUPS.includes(group.value) ? empty().scope : filter.scope,
  }))

  /** Keys that change what this tab shows. */
  const applies = (key: Key) => {
    if (UNFILTERED_GROUPS.includes(group.value)) return false
    if (key === 'real') return realTerms.value
    if (key === 'store' || key === 'cur') return !ALL_WALLETS_GROUPS.includes(group.value)
    return true
  }
  /** How many fields narrow this tab: «Сбросить (2)» shows when any. */
  const narrowed = computed(() => KEYS.filter((key) => applies(key) && one(key)).length)
  function reset() {
    Object.assign(filter, empty())
  }

  return { filter, range, fallback, effective, realTerms, narrowed, reset }
}
