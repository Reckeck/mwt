import { computed } from 'vue'
import { refDebounced } from '@vueuse/core'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { txId } from '@/shared/lib/transactions'

import { formToInput } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'

/** Balances before and after the operation, while it is being typed. */
export function useTxPreview(state: TxFormState) {
  const { props, form, editing } = state

  // Serialized so the debounce and the query key change only when the draft does.
  const draftKey = computed(() => {
    if (props.template) return null
    // As soon as the money side is known: the comment and categories do not move balances.
    const input = formToInput(form)
    if (!input || !('amount' in input ? Number(input.amount) > 0 : true)) return null
    return JSON.stringify(input)
  })
  const debouncedDraft = refDebounced(draftKey, 300)
  const preview = useQuery({
    queryKey: computed(() => ['tx-preview', debouncedDraft.value, props.transaction ? txId(props.transaction) : null]),
    queryFn: () =>
      call<TxPreview>('preview_transaction', {
        input: {
          transaction: JSON.parse(debouncedDraft.value ?? 'null') as TransactionInput,
          replaces: editing.value && props.transaction ? txId(props.transaction) : null,
        },
      }),
    enabled: computed(() => debouncedDraft.value !== null),
    placeholderData: (previous) => previous,
    staleTime: 5_000,
  })

  return { debouncedDraft, preview }
}
