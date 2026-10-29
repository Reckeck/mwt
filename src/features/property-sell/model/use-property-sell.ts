import { type Ref, computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { refDebounced } from '@vueuse/core'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { fiatRef } from '@/shared/lib/asset-ref'
import { toIsoTimestamp } from '@/shared/lib/datetime'
import { decimalInput, parseDecimal } from '@/shared/lib/decimal'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { LEDGER_KEYS, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'

const ident = (asset: AssetRef): AssetIdent => ({
  symbol: asset.symbol,
  name: asset.name,
  coingecko_id: asset.coingecko_id,
  chain: asset.chain,
  contract: asset.contract,
})

/**
 * `Sell "Car"` (Wallets-4 · 21): the whole property sold for money into
 * a wallet, on a day. What it stood at in the books and the gain over it come
 * from the backend's preview; the wallet goes to the archive when asked.
 */
export function usePropertySell(store: () => Store, positions: () => StorePosition[], open: Ref<boolean>) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const tree = useStoreTree()
  /** Money lands in a live wallet of money, never in another property. */
  const nodes = computed(() =>
    flattenStoreTree(tree.data.value ?? []).filter(
      (node) => !node.store.archived && node.store.kind !== 'property' && node.store.id !== store().id,
    ),
  )
  /** The thing itself: the property's one asset that is not money. */
  const held = computed(
    () => positions().find((position) => !position.asset.chain && Number(position.quantity) > 0) ?? null,
  )
  const values = useQuery({
    // no placeholder: another property's money must not price this one
    queryKey: computed(() => ['property-valuations', store().id]),
    queryFn: () => call<PropertyValue[]>('property_valuations', { storeId: store().id }),
    enabled: open,
  })
  /** The money the property is valued in; the price typed is in it. */
  const currency = computed(
    () =>
      values.data.value?.find((row) => row.symbol === held.value?.asset.symbol)?.currency ??
      store().property?.currency ??
      'usd',
  )

  const received = ref('')
  const proceedsTo = ref('')
  const date = ref(localDate(new Date()))
  const archive = ref(true)
  watch(open, (isOpen) => {
    if (!isOpen) return
    received.value = ''
    proceedsTo.value =
      nodes.value.find((node) => node.store.kind === 'bank')?.store.id ?? nodes.value[0]?.store.id ?? ''
    date.value = localDate(new Date())
    archive.value = true
  })

  /** One unit at the typed sum; several units share it. */
  function input(sum: string): TransactionInput | null {
    const thing = held.value
    if (!thing) return null
    const quantity = Number(thing.quantity)
    const unitPrice = quantity === 1 ? sum : decimalInput(Number(sum) / quantity, 8)
    return {
      type: 'asset_sell',
      common: {
        timestamp: date.value === localDate(new Date()) ? null : toIsoTimestamp(`${date.value}T12:00`),
        comment: '',
        categories: [],
        flow_id: null,
        contact_id: null,
        price_usd: null,
        fx_override: null,
        price: null,
        price_currency: null,
      },
      store_id: store().id,
      asset: ident(thing.asset),
      unit_price: unitPrice,
      quantity: thing.quantity,
      currency: ident(fiatRef(currency.value)),
      proceeds_to: proceedsTo.value || null,
      fee: null,
    }
  }

  // The book value shows before anything is typed: a zero price still previews it.
  const draft = computed(() => JSON.stringify(input(parseDecimal(received.value) ?? '0')))
  const debounced = refDebounced(draft, 300)
  const preview = useQuery({
    queryKey: computed(() => ['tx-preview', debounced.value]),
    queryFn: () =>
      call<TxPreview>('preview_transaction', { input: { transaction: JSON.parse(debounced.value), replaces: null } }),
    enabled: computed(() => open.value && held.value !== null),
    placeholderData: (previous) => previous,
  })
  const sale = computed(() => preview.data.value?.sale ?? null)
  /** Shown once a sum is typed: before that the gain would be minus the whole value. */
  const gain = computed(() =>
    parseDecimal(received.value) && Number(received.value) > 0 ? (sale.value?.gain ?? null) : null,
  )

  const form = useFormValidation(
    () => ({
      held: held.value ? null : t('propertySell.nothing'),
      received: positiveAmount(received.value),
      proceedsTo: required(proceedsTo.value, 'pick'),
      date: required(date.value),
    }),
    { resetOn: [open] },
  )

  const save = useMutation({
    mutationFn: async () => {
      const body = input(parseDecimal(received.value) ?? '')
      if (!body) throw { code: 'INVALID_INPUT', message: 'nothing to sell' }
      await call<Transaction>('add_transaction', { input: body })
      if (archive.value) await call('archive_store', { id: store().id, archived: true })
    },
    onSuccess: () => {
      for (const queryKey of [...LEDGER_KEYS, ['stores'], ['store'], ['property-valuations']])
        void queryClient.invalidateQueries({ queryKey })
      open.value = false
    },
    meta: {
      error: 'propertySell.failed',
      success: () => t(archive.value ? 'propertySell.soldArchived' : 'propertySell.sold', { name: store().name }),
    },
  })

  return { nodes, held, currency, received, proceedsTo, date, archive, sale, gain, form, save }
}
