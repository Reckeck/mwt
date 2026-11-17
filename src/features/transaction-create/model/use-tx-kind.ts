import { computed, nextTick, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { emptyForm } from '../transaction-form.model'
import { applyPlan } from '../lib/form/apply-plan'
import { applyPropsLinks, type TxFormState } from './use-tx-form-state'

/**
 * Which kinds the wallet offers and which one the form is on: the wallet's
 * default, a transfer to a contact, an automation preset or a calculator plan.
 * Trades and DeFi actions are never offered for a new record (spec E14).
 */
export function useTxKind(state: TxFormState) {
  const { t } = useI18n()
  const { props, form, editing, storeKind, isProperty } = state

  /** Which kinds this wallet accepts. The binding lives in Rust; the form only asks. */
  const kinds = useQuery({
    // no placeholder: a watcher picks the form kind from this wallet's kinds
    queryKey: computed(() => ['transaction-kinds', storeKind.value]),
    queryFn: () => call<TransactionKinds>('transaction_kinds_for_store_kind', { kind: storeKind.value }),
    enabled: computed(() => storeKind.value !== null),
  })

  const offered = computed<TxKind[]>(() => {
    if (editing.value && props.transaction) return [form.kind]
    const data = kinds.data.value
    if (props.template) return data?.verbs ?? []
    return data ? [...data.verbs, ...data.special] : []
  })

  // A new form lands on the wallet's default kind once it is known — or on a
  // transfer to the given side, when it was opened from a contact.
  watch(
    () => kinds.data.value,
    (data) => {
      if (!data || editing.value) return
      if (props.counterpart && offered.value.includes('transfer')) {
        form.kind = 'transfer'
        // Money comes back from the contact when I borrow or collect; it goes to them otherwise.
        if (props.debt === 'borrow' || props.debt === 'collect') {
          form.from = props.counterpart
          form.to = { store: props.storeId }
        } else {
          form.to = props.counterpart
        }
        if (props.debt) form.debt = props.debt
      } else if (props.initial && offered.value.includes(props.initial.kind) && !props.transaction) {
        form.kind = props.initial.kind
        if (!form.categories.length) form.categories = [...props.initial.categories]
        if (props.initial.to && props.initial.to !== props.storeId) form.to = { store: props.initial.to }
      } else if (!offered.value.includes(form.kind)) {
        // Repeat of a kind this wallet does not offer: the wallet's normal new form, not a switched copy.
        if (props.prefill) {
          Object.assign(form, emptyForm(props.storeId, data.default))
          applyPropsLinks(form, props)
        } else form.kind = data.default
      }
    },
    { immediate: true },
  )

  /** The kind always has a value: pressing the active one again is ignored. */
  function pickKind(value: unknown) {
    if (typeof value !== 'string' || !value) return
    form.kind = value as TxKind
    // Focus follows the tab: what is typed next goes to the amount (the typed one stays), not to nowhere.
    void nextTick(() => document.getElementById('tx-amount')?.focus())
  }

  const verbLabel = (kind: TxKind) =>
    kind === 'topup' && isProperty.value ? t('tx.verb.acquire') : t(`tx.kind.${kind}`)

  /* --- calculator plan, applied once per new object --- */
  let appliedPlan: unknown = null
  watch(
    () => [props.plan, kinds.data.value] as const,
    ([plan, data]) => {
      if (!plan || !data || plan === appliedPlan || editing.value) return
      appliedPlan = plan
      applyPlan(form, plan, offered.value)
    },
    { immediate: true },
  )

  return { kinds, offered, pickKind, verbLabel }
}
