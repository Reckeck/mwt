import { computed, reactive, ref, watch } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import type { StagedAttachment } from '@/entities/attachment'
import { call } from '@/shared/api/ipc'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useEditBase } from '@/shared/lib/form'
import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { txId, txStoreId } from '@/shared/lib/transactions'

import { emptyForm, formFromTransaction } from '../transaction-form.model'
import type { TransactionFormProps } from '../transaction-form.types'
import { TX_PROPERTY_LINKS_KEY, useContacts, useStoreTree } from '@/shared/api/queries'
import { useSharedEvents } from '@/entities/shared-event'

/** The form's data and what it reads about the wallet it is opened on. */
export function useTxFormState(props: TransactionFormProps) {
  const editing = computed(() => props.transaction !== null && !props.template)

  const tree = useStoreTree()
  const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
  /** Cards and accounts a cashback can come from. */
  const cashbackSources = computed(() =>
    nodes.value.filter((node) => !node.store.archived && node.store.kind === 'bank'),
  )
  /** Live property an operation can belong to (the `Property` field). */
  const properties = computed(() =>
    nodes.value.filter((node) => !node.store.archived && node.store.kind === 'property'),
  )
  const contextStore = computed(() => nodes.value.find((node) => node.store.id === props.storeId)?.store ?? null)
  const storeKind = computed(() => contextStore.value?.kind ?? null)
  const isProperty = computed(() => storeKind.value === 'property')
  const isCrypto = computed(() => ['exchange', 'crypto'].includes(storeKind.value ?? ''))

  const form = reactive(
    props.transaction
      ? formFromTransaction(props.transaction, props.storeId)
      : props.prefill
        ? formFromTransaction(props.prefill, props.storeId)
        : emptyForm(props.storeId, 'payment'),
  )
  // Continuing a chain: the new operation joins it.
  if (props.flowId && !props.transaction) form.flowId = props.flowId
  if (props.propertyId) form.propertyId = props.propertyId
  if (props.initial && !props.transaction) {
    if (props.initial.amount) form.amount = props.initial.amount
    if (props.initial.comment) form.comment = props.initial.comment
  }

  /**
   * B3: the operation as the sync keeps it now (re-read after `vault-synced`; the prop may be a
   * copy made once). The form sends what it opened with as `base` and offers `Refresh`.
   */
  const liveRows = useQuery({
    // no placeholder: another wallet's rows say nothing about this operation
    queryKey: computed(() => ['transactions', 'edit-live', props.transaction ? txStoreId(props.transaction) : '']),
    queryFn: () =>
      call<Transaction[]>('list_transactions', {
        filter: emptyTxFilter({
          scope: { store_ids: [txStoreId(props.transaction as Transaction)], include_children: false },
        }),
      }),
    enabled: editing,
  })
  const edit = useEditBase({
    open: true,
    entity: () => (editing.value ? props.transaction : null),
    live: () => {
      const id = props.transaction ? txId(props.transaction) : ''
      return liveRows.data.value?.find((row) => txId(row) === id)
    },
    model: (entity: Transaction) => ({ ...formFromTransaction(entity, props.storeId) }),
  })
  function refreshFromOtherDevice() {
    edit.refresh(
      () => ({ ...form }),
      (model) => Object.assign(form, model),
    )
  }

  /** What this wallet holds: property to sell, and the asset a new operation starts with. */
  const overview = useQuery({
    // no placeholder: the form reads this wallet's assets, not the last one's
    queryKey: computed(() => ['store', props.storeId, false]),
    queryFn: () => call<StoreOverview>('get_store_overview', { storeId: props.storeId, includeChildren: false }),
  })
  const heldAssets = computed(() => overview.data.value?.positions.map((position) => position.asset) ?? [])

  const contacts = useContacts()

  /** The edited operation's property link lives beside it in the vault, not inside it. */
  const links = useQuery({
    queryKey: TX_PROPERTY_LINKS_KEY,
    queryFn: () => call<TxProperty[]>('tx_property_links'),
    enabled: editing,
  })
  watch(
    () => links.data.value,
    (rows) => {
      if (!props.transaction || form.propertyId) return
      const id = txId(props.transaction)
      form.propertyId = rows?.find((row) => row.tx_id === id)?.property_id ?? ''
    },
    { immediate: true },
  )

  /**
   * The shared event a new payment belongs to: once saved, it is linked as a purchase.
   * Offered for a new payment only, and only for events I take part in.
   */
  const sharedEventId = ref('')
  const events = useSharedEvents(false)
  const sharedEventOptions = computed(() =>
    editing.value || props.template || form.kind !== 'payment'
      ? []
      : (events.data.value ?? []).filter((row) => row.i_participate && row.status !== 'closed'),
  )

  /** Files and links of a new operation (Attach 5): attached once it is saved, dropped with the form. */
  const attachments = ref<StagedAttachment[]>([])

  /** What the backend rejected, by field (`Pick another wallet`): shown under it until the form is edited. */
  const serverErrors = ref<Record<string, string>>({})
  watch(form, () => (serverErrors.value = {}), { deep: true })

  /** Every live store but the one the money leaves, for AssetSell proceeds. */
  const proceedsStores = computed(() =>
    nodes.value.filter((node) => !node.store.archived && node.store.id !== props.storeId),
  )

  return {
    props,
    editing,
    nodes,
    cashbackSources,
    properties,
    contextStore,
    storeKind,
    isProperty,
    isCrypto,
    form,
    editBase: edit.base,
    changedElsewhere: edit.changedElsewhere,
    refreshFromOtherDevice,
    overview,
    heldAssets,
    contacts,
    proceedsStores,
    attachments,
    serverErrors,
    sharedEventId,
    sharedEventOptions,
  }
}

export type TxFormState = ReturnType<typeof useTxFormState>
