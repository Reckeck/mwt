import { type MaybeRefOrGetter, computed, toValue } from 'vue'

import { useReport } from '@/entities/report'
import { presetRange } from '@/shared/lib/period'

import type { NetWorthSpan } from '../net-worth.types'

/** The capital report (`networth`) over a span: its points, the report's currency and the query state. */
export function useNetWorth(span: MaybeRefOrGetter<NetWorthSpan>) {
  const filter = computed<ReportFilter>(() => {
    const range = presetRange(toValue(span))
    return {
      scope: { store_ids: [], include_children: true },
      from: range.from,
      to: range.to,
      year: Number(range.to.slice(0, 4)),
      currency: null,
      real_terms: false,
    }
  })
  const report = useReport('networth', filter)
  const worth = computed(() =>
    report.data.value && 'networth' in report.data.value ? report.data.value.networth : null,
  )
  return {
    points: computed(() => worth.value?.points ?? []),
    currency: computed(() => worth.value?.meta.currency ?? ''),
    hasData: computed(() => worth.value?.has_data ?? false),
    loading: computed(() => report.isPending.value),
    error: computed(() => report.error.value),
    refetch: () => report.refetch(),
  }
}
