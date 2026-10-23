import { computed, ref, toRaw, watch, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * What «Ещё фильтры» edits: a copy of the filter taken when the panel opens.
 * The list stays as it was until «Показать N» applies it; N is how many
 * operations the draft would show, counted by the same command as the list.
 */
export function useFilterDraft(filter: Ref<TxFilter>, open: Ref<boolean>) {
  const draft = ref<TxFilter>(structuredClone(toRaw(filter.value)))
  watch(open, (now) => {
    if (now) draft.value = structuredClone(toRaw(filter.value))
  })

  const count = useQuery({
    queryKey: computed(() => ['transactions', 'page', 'count', draft.value]),
    queryFn: () => call<TxPage>('list_transactions_page', { filter: draft.value, offset: 0, limit: 1 }),
    enabled: open,
    placeholderData: (previous) => previous,
  })
  const total = computed(() => count.data.value?.total)

  function apply() {
    filter.value = structuredClone(toRaw(draft.value))
  }
  return { draft, total, apply }
}
