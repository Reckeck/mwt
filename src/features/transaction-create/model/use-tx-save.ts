import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { txChainAndHash, txId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import { emptyForm, formToInput, formToInputs, isLinked } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'

/** Every query a saved operation can show up in. */
const STALE_KEYS = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances']]

/**
 * Writing the operation: one, or a linked chain when the money lands on
 * several accounts; a new form then starts blank on the same kind and asset.
 */
export function useTxSave(state: TxFormState, onSaved: (tx: Transaction) => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settings = useSettingsStore()
  const lookup = useTradeLookupStore()
  const { props, form, editing } = state

  const linked = computed(() => !props.template && isLinked(form))

  const save = useMutation({
    mutationFn: async () => {
      if (linked.value && !props.transaction) {
        const inputs = formToInputs(form)
        if (!inputs) throw { code: 'INVALID_INPUT', message: t('form.assetRequired') }
        const saved = await call<Transaction[]>('add_linked_transactions', {
          input: { transactions: inputs, flow_id: form.flowId.trim() || null } satisfies AddLinkedTransactionsInput,
        })
        return saved[0]
      }
      const input = formToInput(form)
      if (!input) throw { code: 'INVALID_INPUT', message: t('form.assetRequired') }
      return props.transaction
        ? call<Transaction>('update_transaction', { input: { id: txId(props.transaction), transaction: input } })
        : call<Transaction>('add_transaction', { input })
    },
    onSuccess: (tx) => {
      for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
      if (!editing.value)
        Object.assign(form, emptyForm(props.storeId, form.kind), { kind: form.kind, asset: form.asset })
      lookup.reset()
      // The first operation with a hash asks once whether it may be looked up.
      if (txChainAndHash(tx).hash && settings.app && settings.app.enrichment_opt_in === null) {
        settings.enrichmentConsentOpen = true
      }
      onSaved(tx)
    },
    meta: {
      error: 'form.saveFailed',
      // Read before `onSuccess` resets the form: the cache's handler runs first.
      success: () =>
        editing.value
          ? t('tx.updated')
          : linked.value
            ? t('tx.split.saved', { count: form.extra.length + 1 })
            : t('form.saved'),
    },
  })

  return { save }
}
