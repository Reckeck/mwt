import { type MaybeRefOrGetter, computed, toValue } from 'vue'

import { useReport, wealthChange, wealthDriver, wealthRows } from '@/entities/report'
import { defaultRange } from '@/shared/lib/period'

/**
 * What «Капитал» opens (Paths 1 · 3): the change over the month, the line of the
 * last year, what it is made of and the group that moved it most. The same
 * networth report Capital draws, so the glance reads it from the cache.
 */
export function useCapitalGlance(enabled: MaybeRefOrGetter<boolean>) {
  const filter = computed<ReportFilter>(() => {
    const range = defaultRange()
    return {
      scope: { store_ids: [], include_children: true },
      from: range.from,
      to: range.to,
      year: Number(range.to.slice(0, 4)),
      currency: null,
      real_terms: false,
    }
  })
  const report = useReport('networth', filter, { enabled: () => toValue(enabled) })
  const worth = computed(() =>
    report.data.value && 'networth' in report.data.value ? report.data.value.networth : null,
  )
  const points = computed(() => worth.value?.points ?? [])
  return {
    currency: computed(() => worth.value?.meta.currency ?? ''),
    total: computed(() => points.value.at(-1)?.net ?? '0'),
    line: computed(() => points.value.map((point) => point.net)),
    change: computed(() => wealthChange(points.value)),
    rows: computed(() => wealthRows(points.value.at(-1))),
    driver: computed(() => wealthDriver(points.value)),
    loading: computed(() => report.isPending.value),
  }
}
