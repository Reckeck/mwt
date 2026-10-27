import { computed, ref, watch, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * A plain copy of a filter. Not `structuredClone(toRaw(..))`: after a field edit the filter is
 * `{ ...filter, x }`, whose nested `scope` and `kinds` are still reactive proxies, and those
 * cannot be structured-cloned (DataCloneError: «Показать N» did nothing).
 */
const copy = (filter: TxFilter): TxFilter => JSON.parse(JSON.stringify(filter))

/**
 * What «Ещё фильтры» edits: a copy of the filter taken when the panel opens.
 * The list stays as it was until «Показать N» applies it; N is how many
 * operations the draft would show, counted by the same command as the list.
 */
export function useFilterDraft(
  filter: Ref<TxFilter>,
  open: Ref<boolean>,
  rowKeys: () => readonly (keyof TxFilter)[] = () => [],
) {
  const draft = ref<TxFilter>(copy(filter.value))
  watch(open, (now) => {
    if (now) draft.value = copy(filter.value)
  })

  // Fields kept in the row (a computer's wallet and kind, the search, report chips) change under the
  // open panel: the draft follows them, or «Показать» would put the old values back.
  watch(
    () => copy(filter.value),
    (now) => {
      if (!open.value) return
      const next = { ...draft.value }
      for (const key of rowKeys()) (next as Record<string, unknown>)[key] = now[key]
      draft.value = next
    },
  )

  const count = useQuery({
    queryKey: computed(() => ['transactions', 'page', 'count', draft.value]),
    queryFn: () => call<TxPage>('list_transactions_page', { filter: draft.value, offset: 0, limit: 1 }),
    enabled: open,
    placeholderData: (previous) => previous,
  })
  const total = computed(() => count.data.value?.total)

  function apply() {
    filter.value = copy(draft.value)
  }
  return { draft, total, apply }
}
