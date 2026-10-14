import { computed, reactive, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const KEYS = ['store', 'from', 'to', 'year', 'cur', 'real'] as const

const empty = (): ReportFilter => ({
  scope: { store_ids: [], include_children: true },
  from: null,
  to: null,
  year: null,
  currency: null,
  real_terms: false,
})

/**
 * One filter for every report tab, kept in the address (`?from=2026-01&store=…`)
 * so switching tabs, Back and a reload keep the period and wallets.
 */
export function useReportFilter() {
  const route = useRoute()
  const router = useRouter()
  const one = (key: string) => (typeof route.query[key] === 'string' ? (route.query[key] as string) : null)

  const filter = reactive<ReportFilter>(empty())
  const store = one('store')
  if (store) filter.scope = { store_ids: [store], include_children: true }
  filter.from = one('from')
  filter.to = one('to')
  filter.year = Number(one('year')) || null
  filter.currency = one('cur')
  filter.real_terms = one('real') === '1'

  watch(
    filter,
    (value) => {
      const query: Record<string, unknown> = { ...route.query }
      for (const key of KEYS) delete query[key]
      const next: Record<string, string | null> = {
        store: value.scope.store_ids[0] ?? null,
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

  /** How many fields narrow the reports: «Сбросить (2)» shows when any. */
  const narrowed = computed(() => KEYS.filter((key) => one(key)).length)
  function reset() {
    Object.assign(filter, empty())
  }

  return { filter, narrowed, reset }
}
