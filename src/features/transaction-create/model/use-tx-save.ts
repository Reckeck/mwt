import { computed, nextTick, toRaw } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { attachStaged } from '@/entities/attachment'
import { sharedEventKeys } from '@/entities/shared-event'
import { call, parseError } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { txChainAndHash, txId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import { coreErrorField } from '../lib/form'
import { emptyForm, formToInput, formToInputs, isLinked } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'

/** Every query a saved operation can show up in. */
const STALE_KEYS = [
  ...LEDGER_KEYS,
  ...PLANNING_KEYS,
  ['contacts', 'balances'],
  ['contacts', 'summaries'],
  ['entity-card'],
]

/**
 * Writing the operation: one, or a linked chain when the money lands on
 * several accounts; a new form then starts blank on the same kind and asset.
 */
export function useTxSave(state: TxFormState, onSaved: (tx: Transaction) => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settings = useSettingsStore()
  const lookup = useTradeLookupStore()
  const { props, form, editing, attachments, serverErrors, sharedEventId, editBase } = state

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
      ? call<Transaction>('update_transaction', {
          input: { id: txId(props.transaction), transaction: input, base: editing.value ? editBase.value : undefined },
        })
      : call<Transaction>('add_transaction', { input })
  }

  /** The operation is saved either way; what did not attach is named in a toast. */
  async function attach(tx: Transaction) {
    if (editing.value || !attachments.value.length) return
    const missed = await attachStaged({ kind: 'tx', id: txId(tx) }, attachments.value)
    void queryClient.invalidateQueries({ queryKey: ['attachments'] })
    if (missed.length) notify.error(null, t('attachments.stagedMissed', { names: missed.join(', ') }))
  }

  /** The payment is saved either way; a refusal of the event (a crypto asset, a closed event) is named in a toast. */
  async function linkToEvent(tx: Transaction) {
    const eventId = sharedEventId.value
    if (!eventId || editing.value || form.kind !== 'payment') return
    try {
      await call<SharedEventView>('attach_payment_to_shared_event', {
        input: {
          event_id: eventId,
          tx_id: txId(tx),
          item_id: null,
          new_item_title: form.comment.trim() || t('tx.field.eventItem'),
          qty: null,
        } satisfies AttachPaymentInput,
      })
      void queryClient.invalidateQueries({ queryKey: sharedEventKeys.all })
      notify.success('tx.field.eventLinked')
    } catch (err) {
      notify.error(err, 'tx.field.eventLinkFailed')
    }
  }

  const save = useMutation({
    mutationFn: async () => {
      let tx: Transaction
      try {
        tx = await write()
      } catch (err) {
        // A rule the core names for one field is told under it, not in a toast.
        const hit = coreErrorField(parseError(err).message, form.kind)
        if (!hit) throw err
        serverErrors.value = { [hit.field]: t(hit.key) }
        void nextTick(() => {
          const field = document.querySelector<HTMLElement>('#transaction-form [data-invalid]')
          field?.querySelector<HTMLElement>('input, button, [tabindex]')?.focus()
          field?.scrollIntoView({ block: 'nearest' })
        })
        return null
      }
      await attach(tx)
      await linkToEvent(tx)
      return tx
    },
    onSuccess: (tx) => {
      if (!tx) return
      attachments.value = []
      sharedEventId.value = ''
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
      // The short form's dialog says it itself, with `Open` or `Undo` (OpForm-1 · 4).
      success: (tx) => {
        // Nothing saved: the core's rule was told under its field.
        if (!tx) return ''
        if (editing.value) return t('tx.updated')
        // A chain says how many operations it became, even from the short form: one click wrote several.
        if (linked.value) return t('tx.split.saved', { count: form.extra.length + 1 })
        return props.compact ? '' : t('form.saved')
      },
    },
  })

  return { save }
}
