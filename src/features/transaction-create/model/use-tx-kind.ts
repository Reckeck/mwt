import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import type { TxFormState } from './use-tx-form-state'

/**
 * Which kinds the wallet offers and which one the form is on: the wallet's
 * default, a transfer to a contact, an automation preset or a calculator plan;
 * trades go to the exchange's Unified account.
 */
export function useTxKind(state: TxFormState) {
  const { t } = useI18n()
  const { props, form, editing, storeKind, contextStore, nodes, isProperty } = state

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
      } else if (!offered.value.includes(form.kind)) form.kind = data.default
    },
    { immediate: true },
  )

  /**
   * Spot and futures live on the exchange's Unified Trading account: a trade
   * entered on the wallet itself or on its funding or earn account is booked
   * there. An older spot or futures sub-account keeps its own trades.
   */
  const unifiedAccount = computed(() => {
    const store = contextStore.value
    if (store?.kind !== 'exchange') return null
    const market = typeof store.binding === 'object' && 'market' in store.binding ? store.binding.market : null
    if (market === 'spot' || market === 'futures' || market === 'unified') return null
    const rootId = store.parent_id ?? store.id
    return (
      nodes.value.find(
        (node) =>
          node.store.parent_id === rootId &&
          !node.store.archived &&
          typeof node.store.binding === 'object' &&
          'market' in node.store.binding &&
          node.store.binding.market === 'unified',
      )?.store ?? null
    )
  })
  const tradeOnUnified = computed(
    () => !editing.value && Boolean(unifiedAccount.value) && ['trade_spot', 'trade_futures'].includes(form.kind),
  )
  watch(
    [tradeOnUnified, () => form.storeId],
    ([onUnified]) => {
      if (editing.value) return
      const target = onUnified && unifiedAccount.value ? unifiedAccount.value.id : props.storeId
      if (form.storeId !== target) form.storeId = target
    },
    { immediate: true },
  )

  /** The kind always has a value: pressing the active one again is ignored. */
  function pickKind(value: unknown) {
    if (typeof value === 'string' && value) form.kind = value as TxKind
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
      if (plan.from && plan.to && offered.value.includes('trade_spot')) {
        // A position is a spot buy: the amount given, and the units it bought.
        form.kind = 'trade_spot'
        form.soldAsset = plan.from
        form.boughtAsset = plan.to
        form.soldQty = plan.amount
        if (plan.mode === 'position' && plan.quantity) form.boughtQty = plan.quantity
      } else if (plan.from) {
        form.kind = offered.value.includes('topup') ? 'topup' : form.kind
        form.asset = plan.from
        form.amount = plan.amount
      }
      if (plan.price) {
        form.price = plan.price
        form.priceCurrency = 'usd'
      }
    },
    { immediate: true },
  )

  return { kinds, offered, unifiedAccount, tradeOnUnified, pickKind, verbLabel }
}
