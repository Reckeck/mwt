import { computed, reactive } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

import { emptyForm, formFromTransaction } from '../transaction-form.model'
import type { TransactionFormProps } from '../transaction-form.types'

/** The form's data and what it reads about the wallet it is opened on. */
export function useTxFormState(props: TransactionFormProps) {
  const editing = computed(() => props.transaction !== null && !props.template)

  const tree = useQuery({
    queryKey: STORE_TREE_QUERY_KEY,
    queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
  })
  const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
  /** Cards and accounts a cashback can come from. */
  const cashbackSources = computed(() =>
    nodes.value.filter((node) => !node.store.archived && node.store.kind === 'bank'),
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

  /** What this wallet holds: property to sell, and the asset a new operation starts with. */
  const overview = useQuery({
    queryKey: computed(() => ['store', props.storeId, false]),
    queryFn: () => call<StoreOverview>('get_store_overview', { storeId: props.storeId, includeChildren: false }),
  })
  const heldAssets = computed(() => overview.data.value?.positions.map((position) => position.asset) ?? [])

  const contacts = useQuery({
    queryKey: ['contacts'],
    queryFn: () => call<Contact[]>('list_contacts'),
  })

  /** Every live store but the one the money leaves, for AssetSell proceeds. */
  const proceedsStores = computed(() =>
    nodes.value.filter((node) => !node.store.archived && node.store.id !== props.storeId),
  )

  return {
    props,
    editing,
    nodes,
    cashbackSources,
    contextStore,
    storeKind,
    isProperty,
    isCrypto,
    form,
    overview,
    heldAssets,
    contacts,
    proceedsStores,
  }
}

export type TxFormState = ReturnType<typeof useTxFormState>
