import { computed, toValue, type MaybeRefOrGetter } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** One report for a kind and a filter; the filter is deep-copied into the key. */
export function useReport(kind: MaybeRefOrGetter<ReportKind>, filter: MaybeRefOrGetter<ReportFilter>) {
  return useQuery({
    queryKey: computed(() => ['report', toValue(kind), JSON.parse(JSON.stringify(toValue(filter)))]),
    queryFn: () => call<AnalyticsReport>('get_report', { kind: toValue(kind), filter: toValue(filter) }),
  })
}

/** Short month label, «мар 26». */
export function monthLabel(key: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { month: 'short', year: '2-digit' }).format(new Date(`${key}-01T12:00`))
}
