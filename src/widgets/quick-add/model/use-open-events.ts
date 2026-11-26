import { computed } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** The open shared events I take part in: where a new expense can be linked as a purchase. */
export function useOpenEvents() {
  const events = useQuery({
    queryKey: ['shared-events', 'list', false],
    queryFn: () => call<SharedEventSummary[]>('list_shared_events', { filter: { include_archived: false } }),
    placeholderData: keepPreviousData,
  })
  return computed(() => (events.data.value ?? []).filter((row) => row.i_participate && row.status !== 'closed'))
}
