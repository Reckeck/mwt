import { computed, toRaw } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { attachStaged } from '@/entities/attachment'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { txChainAndHash, txId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import { emptyForm, formToInput, formToInputs, isLinked } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'

/** Every query a saved operation can show up in. */
const STALE_KEYS = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances'], ['entity-card']]

/**
 * Writing the operation: one, or a linked chain when the money lands on
 * several accounts; a new form then starts blank on the same kind and asset.
 */
export function useTxSave(state: TxFormState, onSaved: (tx: Transaction) => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settings = useSettingsStore()
  const lookup = useTradeLookupStore()
  const { props, form, editing, attachments } = state

  const linked = computed(() => !props.template && isLinked(form))

  /** The operation itself; one of a linked chain stands for it. */
  async function write(): Promise<Transaction> {
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
  }

  /** The operation is saved either way; what did not attach is named in a toast. */
  async function attach(tx: Transaction) {
    if (editing.value || !attachments.value.length) return
    const missed = await attachStaged({ kind: 'tx', id: txId(tx) }, attachments.value)
    void queryClient.invalidateQueries({ queryKey: ['attachments'] })
    if (missed.length) notify.error(null, t('attachments.stagedMissed', { names: missed.join(', ') }))
  }

  const save = useMutation({
    mutationFn: async () => {
      const tx = await write()
      await attach(tx)
      return tx
    },
    onSuccess: (tx) => {
      attachments.value = []
      for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
      if (!editing.value)
        Object.assign(form, emptyForm(props.storeId, form.kind), {
          kind: form.kind,
          // A copy: the saved asset is the user's now, so a wallet whose largest position
          // changed after this save does not swap it (autofill replaces only its own value).
          asset: form.asset && { ...toRaw(form.asset) },
          // The short form runs in series (Ctrl Enter): the wallet, the category and the date
          // stay for the next one (the toast says so, `quickAdd.kept`).
          ...(props.compact
            ? { timestamp: form.timestamp, storeId: form.storeId, categories: [...form.categories] }
            : {}),
        })
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
      // The short form's dialog says it itself, with «Открыть» or «Отменить» (OpForm-1 · 4).
      success: () =>
        props.compact && !editing.value
          ? ''
          : editing.value
            ? t('tx.updated')
            : linked.value
              ? t('tx.split.saved', { count: form.extra.length + 1 })
              : t('form.saved'),
    },
  })

  return { save }
}
