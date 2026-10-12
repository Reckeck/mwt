<script setup lang="ts">
import { computed, reactive, ref, toRaw, watch } from 'vue'
import { refDebounced } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ArrowUpDown } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { AssetInput } from '@/entities/asset'
import { TxPreviewCard } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call, parseError } from '@/shared/api/ipc'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { formatDateTime, formatMoney } from '@/shared/lib/format'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { txChainAndHash, txId } from '@/shared/lib/transactions'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import { AccountRefPicker } from './ui/account-ref-picker'
import { SplitRows } from './ui/split-rows'
import { useAccountAsset } from './model/use-account-asset'
import { parseDecimal } from '@/shared/lib/decimal'
import { CategorySelect } from '@/features/category-select'
import type { TransactionFormProps } from './transaction-form.types'
import {
  categoryGroupsFor,
  debtChoices,
  emptyForm,
  formFromTransaction,
  formToInput,
  formToInputs,
  isFormComplete,
  isLinked,
  missingFields,
  pricedAsset,
  toIsoTimestamp,
} from './transaction-form.model'

const props = withDefaults(defineProps<TransactionFormProps>(), {
  transaction: null,
  plan: null,
  disabled: false,
  counterpart: null,
  template: false,
  flowId: null,
  prefill: null,
})

/** `saved` after every successful write; `submit` instead in template mode. */
const emit = defineEmits<{ saved: [tx: Transaction]; submit: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const lookup = useTradeLookupStore()
const settings = useSettingsStore()

const editing = computed(() => props.transaction !== null && !props.template)

const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
})

const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
/** Cards and accounts a cashback can come from. */
const cashbackSources = computed(() => nodes.value.filter((node) => !node.store.archived && node.store.kind === 'bank'))
const contextStore = computed(() => nodes.value.find((node) => node.store.id === props.storeId)?.store ?? null)
const storeKind = computed(() => contextStore.value?.kind ?? null)
const isProperty = computed(() => storeKind.value === 'property')
const isCrypto = computed(() => ['exchange', 'crypto'].includes(storeKind.value ?? ''))

/** Which kinds this wallet accepts. The binding lives in Rust; the form only asks. */
const kinds = useQuery({
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

const form = reactive(
  props.transaction
    ? formFromTransaction(props.transaction, props.storeId)
    : props.prefill
      ? formFromTransaction(props.prefill, props.storeId)
      : emptyForm(props.storeId, 'payment'),
)

// Continuing a chain: the new operation joins it.
if (props.flowId && !props.transaction) form.flowId = props.flowId

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

/** Property is named, not searched: a flat is not on CoinGecko. */
const propertyName = ref(form.asset && isProperty.value ? form.asset.name : '')
watch(propertyName, (name) => {
  if (!isProperty.value || form.kind !== 'topup') return
  const symbol = name.trim().toUpperCase().replace(/\s+/g, '_').slice(0, 24)
  form.asset = symbol ? { symbol, name: name.trim(), coingecko_id: null, chain: null, contract: null, quote: symbol } : null
})

/** What this wallet holds: property to sell, and the asset a new operation starts with. */
const overview = useQuery({
  queryKey: computed(() => ['store', props.storeId, false]),
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: props.storeId, includeChildren: false }),
})

/**
 * A new operation starts with the wallet's main asset (its largest position);
 * an empty bank or cash wallet starts with the base currency.
 */
watch(
  () => [overview.data.value, storeKind.value] as const,
  ([data, kind]) => {
    if (editing.value || form.asset || !kind || kind === 'property') return
    const main = data?.positions[0]?.asset
    if (main) form.asset = main
    else if (kind === 'bank' || kind === 'cash') {
      const code = settings.baseCurrency.toUpperCase()
      form.asset = { symbol: code, name: code, coingecko_id: null, chain: 'fiat', contract: null, quote: code }
    }
  },
  { immediate: true },
)
const heldAssets = computed(() => overview.data.value?.positions.map((position) => position.asset) ?? [])
const sellKey = ref(form.kind === 'asset_sell' && form.asset ? form.asset.symbol : '')
watch(sellKey, (symbol) => {
  form.asset = heldAssets.value.find((asset) => asset.symbol === symbol) ?? null
})

const contacts = useQuery({
  queryKey: ['contacts'],
  queryFn: () => call<Contact[]>('list_contacts'),
})

/** Every live store but the one the money leaves, for AssetSell proceeds. */
const proceedsStores = computed(() => nodes.value.filter((node) => !node.store.archived && node.store.id !== props.storeId))

/** Swap the ends of a transfer or conversion. */
function swapSides() {
  const from = form.from
  form.from = form.to
  form.to = from
  if (form.kind === 'convert') {
    const asset = form.fromAsset
    form.fromAsset = form.toAsset
    form.toAsset = asset
    const amount = form.fromAmount
    form.fromAmount = form.toAmount
    form.toAmount = amount
  }
}

/* --- the asset a side starts with, so it is not picked twice --- */
const fromAccount = useAccountAsset(computed(() => form.from))
const toAccount = useAccountAsset(computed(() => form.to))
/** What the form set by itself; a value the user picked is never replaced. */
const autoSet: Partial<Record<'fromAsset' | 'toAsset' | 'asset' | 'soldAsset', AssetRef>> = {}
function autoFill(key: 'fromAsset' | 'toAsset' | 'asset' | 'soldAsset', value: AssetRef | null) {
  if (editing.value || !value) return
  const current = form[key]
  // `form` is reactive: what it hands back is a proxy of what was stored.
  if (current && toRaw(current) !== autoSet[key]) return
  autoSet[key] = toRaw(value)
  form[key] = value
}
// The account money leaves gives its first asset.
watch(
  () => [fromAccount.main.value, form.kind] as const,
  ([main, kind]) => {
    if (kind === 'convert') autoFill('fromAsset', main)
    if (kind === 'transfer') autoFill('asset', main)
  },
  { immediate: true },
)
// Money arriving on a bank or cash account arrives in its currency.
watch(
  () => [toAccount.currency.value, form.kind] as const,
  ([currency, kind]) => {
    if (kind === 'convert' && toAccount.moneyOnly.value) autoFill('toAsset', currency)
  },
  { immediate: true },
)
watch(
  () => [overview.data.value?.positions[0]?.asset ?? null, form.kind] as const,
  ([main, kind]) => {
    if (kind === 'trade_spot') autoFill('soldAsset', main)
  },
  { immediate: true },
)

/* --- the rate and the received amount follow each other --- */
let autoToAmount = ''
watch(
  () => [form.fromAmount, form.rate] as const,
  ([give, rate]) => {
    if (form.kind !== 'convert' || editing.value) return
    const amount = Number(parseDecimal(give) ?? Number.NaN)
    const price = Number(parseDecimal(rate) ?? Number.NaN)
    if (!(amount > 0 && price > 0)) return
    // With several receiving accounts each says its own amount.
    if (form.extra.length || (form.toAmount.trim() && form.toAmount !== autoToAmount)) return
    autoToAmount = String(Number((amount * price).toFixed(8)))
    form.toAmount = autoToAmount
  },
)
/** «1 USDT = 92.5 RUB» from the two amounts, when no rate was typed. */
const impliedRate = computed(() => {
  if (form.kind !== 'convert' || form.rate.trim()) return ''
  const give = Number(parseDecimal(form.fromAmount) ?? Number.NaN)
  const got = receivedTotal.value
  if (!(give > 0 && got > 0) || !form.fromAsset || !form.toAsset) return ''
  const rate = got / give
  return `1 ${form.fromAsset.symbol} = ${Number(rate.toPrecision(8))} ${form.toAsset.symbol}`
})

/* --- P2P: the money goes to a person and comes back from them --- */
function pickConvertMode(value: unknown) {
  if (value !== 'direct' && value !== 'p2p') return
  form.convertMode = value
  // Through someone, both ends are your own accounts.
  if (value === 'p2p' && form.to && !('store' in form.to)) form.to = null
}

/** What is still missing, said under the form. */
const missing = computed(() => (props.template || props.disabled ? [] : missingFields(form)))
const linked = computed(() => !props.template && isLinked(form))
/** Everything the exchange brings in, over all receiving accounts. */
const receivedTotal = computed(() =>
  [form.toAmount, ...form.extra.map((row) => row.receive)].reduce(
    (sum, value) => sum + Number(parseDecimal(value) ?? 0),
    0,
  ),
)

// The first account an exchange lands on is there to pick, not picked for you.
watch(
  () => form.kind,
  (kind) => {
    if (kind === 'transfer' && form.to && 'store' in form.to && form.to.store === props.storeId) form.to = null
  },
  { immediate: true },
)

const debts = computed(() => debtChoices(form))
/** The select holds a string; `''` is "not a debt". */
const debtValue = computed({
  get: () => form.debt ?? '',
  set: (value: string) => {
    form.debt = (value || null) as DebtEffect | null
  },
})
watch(debts, (choices) => {
  if (form.debt && !choices.includes(form.debt)) form.debt = null
})

/** On-chain fields: crypto wallets for topup/payment, a transfer of a non-fiat asset, DeFi. */
const showChain = computed(() => {
  if (props.template) return false
  if (form.kind === 'defi') return true
  if (form.kind === 'transfer') return form.asset?.chain !== 'fiat'
  return isCrypto.value && ['topup', 'payment'].includes(form.kind)
})

// The network follows the picked asset.
watch(
  () => form.asset,
  (asset) => {
    if (asset?.chain && asset.chain !== 'fiat') form.network = asset.chain
  },
)

/* --- the rate a new operation records (plan §6.2) --- */
const priced = computed(() => pricedAsset(form))
const quote = useQuery({
  queryKey: computed(() => [
    'price-at',
    priced.value?.symbol,
    priced.value?.coingecko_id,
    priced.value?.chain,
    priced.value?.contract,
    form.timestamp,
  ]),
  queryFn: () => {
    const asset = priced.value as AssetRef
    return call<CachedPrice>('get_cached_price', {
      input: {
        symbol: asset.symbol,
        coingecko_id: asset.coingecko_id,
        chain: asset.chain,
        contract: asset.contract,
        at: toIsoTimestamp(form.timestamp),
      },
    })
  },
  enabled: computed(() => !props.template && Boolean(priced.value) && !form.price.trim()),
})

/** The currency the price field is in: the user's pick, else the base currency. */
const priceCode = computed(() => (form.priceCurrency || settings.baseCurrency).toLowerCase())

/**
 * The currency the rate hint speaks: the price field's, unless that is the
 * asset itself (1 RUB = 1 RUB says nothing) — then the base currency, or USD
 * when the asset is the base currency.
 */
const hintCode = computed(() => {
  const asset = priced.value?.symbol.toLowerCase() ?? ''
  if (asset !== priceCode.value) return priceCode.value
  const base = settings.baseCurrency.toLowerCase()
  return asset === base ? 'usd' : base
})

/** The cached USD quote, shown in the hint's currency. */
const quoteInCurrency = useQuery({
  queryKey: computed(() => ['convert', quote.data.value?.price_usd, hintCode.value, form.timestamp]),
  queryFn: () =>
    call<ConvertedAmount>('convert_amount', {
      input: {
        from: 'usd',
        to: hintCode.value,
        amount: quote.data.value?.price_usd ?? '0',
        at: toIsoTimestamp(form.timestamp),
      } satisfies ConvertAmountInput,
    }),
  enabled: computed(() => Boolean(quote.data.value?.ts) && hintCode.value !== 'usd'),
})

/**
 * The price is stamped by the backend from the rates of the operation's day;
 * fetch that day's rates (or the last published ones) of the asset and the
 * price currency ahead of time, so the stamp has something to use.
 */
const ensuredFx = new Set<string>()
watch(
  [priceCode, () => form.timestamp, () => form.price, () => priced.value?.symbol],
  ([code, stamp, price, asset]) => {
    if (props.template || (!price.trim() && !asset)) return
    const day = (toIsoTimestamp(stamp) ?? new Date().toISOString()).slice(0, 10)
    const currencies = [...new Set([code, (asset ?? '').toLowerCase()])].filter(
      (entry) => entry && entry !== 'usd' && !ensuredFx.has(`${entry}@${day}`),
    )
    if (!currencies.length) return
    for (const entry of currencies) ensuredFx.add(`${entry}@${day}`)
    void call<EnsureFxOutput>('ensure_fx', { input: { currencies, date: day } satisfies EnsureFxInput }).catch(() => {
      /* offline: the backend answers FX_MISSING on save, with a clear message */
    })
  },
)
/** Which day's rate an empty price field stands for — or that there is none yet. */
const rateHint = computed(() => {
  const data = quote.data.value
  // A dollar is always worth a dollar; saying so is noise.
  if (!priced.value || priced.value.symbol.toLowerCase() === hintCode.value || form.price.trim() || !data) return ''
  if (!data.ts) return t('tx.rate.none')
  const converted = hintCode.value === 'usd' ? data.price_usd : quoteInCurrency.data.value?.amount
  if (!converted) return ''
  const price = Number(converted)
  return t('tx.rate.at', {
    date: formatDateTime(data.ts),
    price: `1 ${priced.value.symbol.toUpperCase()} = ${formatMoney(price, { decimals: price < 1 ? 6 : 2, currency: hintCode.value })}`,
  })
})

/* --- budget limits this payment would break (plan §9.7) --- */
const budgetStatus = useQuery({
  queryKey: ['budget-status'],
  queryFn: () => call<BudgetStatus[]>('get_budget_status'),
  enabled: computed(() => !props.template && form.kind === 'payment'),
})
const overLimits = useQuery({
  queryKey: computed(() => [
    'budget-overrun',
    form.kind,
    form.amount,
    form.asset?.symbol,
    form.categories.join(','),
    budgetStatus.data.value,
  ]),
  queryFn: async () => {
    const amount = Number(form.amount)
    const asset = form.asset
    if (!asset || !(amount > 0)) return []
    const matching = (budgetStatus.data.value ?? []).filter((status) => {
      const target = status.budget.target
      if (target === 'total') return true
      if ('category' in target) return form.categories.includes(target.category)
      return target.store.store_ids.includes(props.storeId)
    })
    const out: Array<{ status: BudgetStatus; over: number }> = []
    for (const status of matching) {
      const converted = await call<ConvertedAmount>('convert_amount', {
        input: { from: asset.symbol, to: status.budget.currency, amount: form.amount, at: null },
      })
      const over = Number(converted.amount ?? 0) - Number(status.remaining)
      if (converted.amount !== null && over > 0) out.push({ status, over })
    }
    return out
  },
  enabled: computed(() => !props.template && form.kind === 'payment' && Boolean(budgetStatus.data.value?.length)),
})
const overrunText = computed(() =>
  (overLimits.data.value ?? [])
    .map(({ status, over }) =>
      t('budget.overrunHint', {
        period: t(`budget.periodKind.${status.budget.period}`),
        amount: formatMoney(over, { currency: status.budget.currency }),
      }),
    )
    .join(' · '),
)

// Moving an operation to another date re-prices it there, unless the price
// was changed by hand in this edit.
const loadedPrice = form.price
watch(
  () => form.timestamp,
  () => {
    if (editing.value && form.price === loadedPrice) form.price = ''
  },
)

/* --- assistants: tx-hash inspection and recent Bybit fills (never override typed values) --- */
/** Text fields an assistant may fill. */
type FillableField = 'feeAmount' | 'price' | 'amount' | 'comment' | 'soldQty' | 'boughtQty' | 'symbol' | 'quantity' | 'fillPrice'

function fillIfEmpty(key: FillableField, value: string | null | undefined) {
  if (!value?.trim() || form[key].trim()) return
  form[key] = value.trim()
}

let hashTimer: ReturnType<typeof setTimeout> | undefined
watch(
  () => form.txHash,
  (value) => {
    if (hashTimer) clearTimeout(hashTimer)
    const hash = value.trim()
    if (!hash) {
      lookup.reset()
      return
    }
    hashTimer = setTimeout(() => void lookup.inspectTx(hash, form.network || undefined), 600)
  },
)

watch(
  () => lookup.hints,
  (hints) => {
    for (const hint of hints) {
      if (hint.field === 'fee') fillIfEmpty('feeAmount', hint.value)
      if (hint.field === 'price' && !form.price.trim()) {
        form.price = hint.value
        form.priceCurrency = 'usd'
      }
      if (hint.field === 'qtyBought' || hint.field === 'qtySold') fillIfEmpty('amount', hint.value)
    }
  },
  { deep: true },
)

const lookupHint = computed(() => {
  if (lookup.loading) return t('tx.lookup.checking')
  if (lookup.notice) return t(`tx.lookup.${lookup.notice}`)
  return null
})

// The fills list needs to know whether this exchange wallet (or a parent) has a key.
watch(
  storeKind,
  (kind) => {
    if (kind === 'exchange') void lookup.loadKeyStatus(props.storeId)
  },
  { immediate: true },
)

const bybitCategory = ref<BybitCategory>('spot')
const showFills = computed(() => lookup.hasKey && (form.kind === 'trade_spot' || form.kind === 'trade_futures'))
watch(
  showFills,
  (shown) => {
    if (shown) void lookup.loadExecutions(form.kind === 'trade_futures' ? 'linear' : bybitCategory.value)
  },
  { immediate: true },
)

async function resolveAsset(symbol: string | null | undefined): Promise<AssetRef | null> {
  if (!symbol) return null
  const hits = await call<AssetSearchHit[]>('search_assets', { query: symbol })
  const hit = hits.find((item) => item.symbol.toUpperCase() === symbol.toUpperCase()) ?? hits[0]
  if (!hit) return null
  const upper = hit.symbol.toUpperCase()
  return { symbol: upper, name: hit.name || upper, coingecko_id: hit.coingecko_id, chain: hit.chain ?? null, contract: hit.contract ?? null, quote: upper }
}

/** Apply a Bybit fill to the form. */
async function applyFill(exec: BybitExecution) {
  const fill = exec.prefill
  fillIfEmpty('comment', fill.comment)
  if (!form.timestamp && fill.occurred_at) form.timestamp = fill.occurred_at.slice(0, 16)
  fillIfEmpty('feeAmount', fill.fee)
  if (form.kind === 'trade_futures') {
    fillIfEmpty('symbol', exec.symbol)
    fillIfEmpty('quantity', exec.exec_qty)
    fillIfEmpty('fillPrice', exec.exec_price)
    form.direction = exec.side.toLowerCase() === 'buy' ? 'long' : 'short'
    return
  }
  fillIfEmpty('soldQty', fill.qty_sold)
  fillIfEmpty('boughtQty', fill.qty_bought)
  // Exchange fills are quoted in USD(T).
  if (!form.price.trim() && fill.price) {
    form.price = fill.price
    form.priceCurrency = 'usd'
  }
  form.liquidity = exec.is_maker ? 'maker' : 'taker'
  const [sold, bought] = await Promise.all([resolveAsset(fill.asset_sold), resolveAsset(fill.asset_bought)])
  if (!form.soldAsset && sold) form.soldAsset = sold
  if (!form.boughtAsset && bought) form.boughtAsset = bought
}

/* --- calculator plan --- */
let appliedPlan: unknown = null
watch(
  () => [props.plan, kinds.data.value] as const,
  ([plan, data]) => {
    if (!plan || !data || plan === appliedPlan || editing.value) return
    appliedPlan = plan
    if (plan.mode === 'converter' && plan.from && plan.to && offered.value.includes('trade_spot')) {
      form.kind = 'trade_spot'
      form.soldAsset = plan.from
      form.boughtAsset = plan.to
      form.soldQty = plan.amount
    } else if (plan.from) {
      form.kind = offered.value.includes('topup') ? 'topup' : form.kind
      form.asset = plan.from
      form.amount = plan.amount
    }
    form.price = plan.price
    form.priceCurrency = 'usd'
  },
  { immediate: true },
)

/* --- before/after preview --- */
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

/* --- save --- */
const STALE_KEYS = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances']]

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
    toast.success(
      editing.value
        ? t('tx.updated')
        : linked.value
          ? t('tx.split.saved', { count: form.extra.length + 1 })
          : t('form.saved'),
    )
    for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
    if (!editing.value) Object.assign(form, emptyForm(props.storeId, form.kind), { kind: form.kind, asset: form.asset })
    lookup.reset()
    // The first operation with a hash asks once whether it may be looked up.
    if (txChainAndHash(tx).hash && settings.app && settings.app.enrichment_opt_in === null) {
      settings.enrichmentConsentOpen = true
    }
    emit('saved', tx)
  },
  onError: (err) => toast.error(t('form.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const valid = computed(() => !props.disabled && isFormComplete(form))

/** Fields entered as numbers: blank, not a number and «≤ 0» each get their own message. */
const AMOUNT_FIELDS = new Set([
  'amount',
  'fromAmount',
  'toAmount',
  'soldQty',
  'boughtQty',
  'quantity',
  'fillPrice',
  'sellQty',
  'sellPrice',
])
/* --- a top-up or a payment needs a category, unless a rule fills it in (backend check V5) --- */
const needsCategory = computed(() => form.kind === 'topup' || form.kind === 'payment')
const rules = useQuery({
  queryKey: ['categorization-rules'],
  queryFn: () => call<CategorizationRule[]>('list_categorization_rules'),
  enabled: needsCategory,
})
/** Same match as `apply_categorization_rules`: a pattern inside the comment or a contact's name. */
const ruleFillsCategory = computed(() => {
  const contactIds = [form.contactId, form.counterpart && 'contact' in form.counterpart ? form.counterpart.contact.contact_id : '']
  const names = (contacts.data.value ?? []).filter((contact) => contactIds.includes(contact.id)).map((contact) => contact.name)
  const haystacks = [form.comment, ...names].map((text) => text.toLowerCase())
  return (rules.data.value ?? []).some(
    (rule) => rule.active && rule.pattern.trim() && haystacks.some((text) => text.includes(rule.pattern.trim().toLowerCase())),
  )
})

/** Every missing field gets its message under it; the list under the form stays as the summary. */
const check = useFormValidation(
  () => {
    if (props.disabled) return {}
    const errors: Record<string, string | null> = {}
    // A template hides the summary list, but its fields are checked all the same.
    for (const key of props.template ? missingFields(form) : missing.value) {
      if (AMOUNT_FIELDS.has(key)) errors[key] = positiveAmount(form[key as keyof typeof form] as string)
      else if (key === 'symbol') errors[key] = required(form.symbol)
      else if (key === 'extra') errors[key] = t('validation.splitRows')
      else errors[key] = required(null, 'pick')
    }
    if (needsCategory.value && !props.template && !form.categories.length && !ruleFillsCategory.value) {
      errors.categories = t('validation.category')
    }
    // Complete on the surface but still not an operation (e.g. an unknown kind of side).
    if (!Object.values(errors).some(Boolean) && !isFormComplete(form)) errors.form = t('validation.summary')
    return errors
  },
  { resetOn: () => form.kind },
)

function submit() {
  if (props.template) emit('submit')
  else save.mutate()
}

/** The dialog owns the submit button, so it reads these; a template's parent reads `input`. */
defineExpose({
  valid,
  /** For a parent that owns the save button of a template: show the messages under the fields. */
  showErrors: () => {
    check.attempted.value = true
  },
  pending: computed(() => save.isPending.value),
  input: computed(() => formToInput(form)),
})

const verbLabel = (kind: TxKind) =>
  kind === 'topup' && isProperty.value ? t('tx.verb.acquire') : t(`tx.kind.${kind}`)
</script>

<template>
  <form
    :id="props.template ? 'transaction-template-form' : 'transaction-form'"
    class="flex flex-col gap-4"
    @submit.prevent="check.submit(submit, $event)"
  >
    <ToggleGroup
      v-if="offered.length > 1"
      type="single"
      variant="outline"
      size="sm"
      class="flex-wrap justify-start"
      :spacing="1"
      :model-value="form.kind"
      :aria-label="t('form.kind')"
      @update:model-value="pickKind"
    >
      <ToggleGroupItem v-for="entry in offered" :key="entry" :value="entry">{{ verbLabel(entry) }}</ToggleGroupItem>
    </ToggleGroup>

    <p v-if="tradeOnUnified && unifiedAccount" class="text-xs text-muted-foreground">
      {{ t('tx.tradeOnUnified', { name: unifiedAccount.name }) }}
    </p>

    <!-- Topup · Payment · DeFi: one asset on this wallet -->
    <template v-if="form.kind === 'topup' || form.kind === 'payment' || form.kind === 'defi'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <div v-if="isProperty && form.kind === 'topup'" class="grid gap-1.5">
          <Label for="tx-property">{{ t('tx.field.property') }}</Label>
          <Input id="tx-property" v-model="propertyName" :placeholder="t('tx.field.propertyHint')" />
        </div>
        <AssetInput v-else v-model="form.asset" required :error="check.error('asset')" :label="t('form.asset')" />
        <FormField for="tx-amount" :label="isProperty ? t('tx.field.units') : t('form.amount')" required :error="check.error('amount')">
          <AmountInput id="tx-amount" v-model="form.amount" :placeholder="t('form.amountHint')" />
        </FormField>
      </div>
      <p
        v-if="overrunText"
        class="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
      >
        {{ overrunText }}
      </p>
      <div v-if="isProperty && form.kind === 'topup'" class="grid gap-1.5">
        <Label for="tx-unit-price">{{ t('tx.field.unitPrice') }}</Label>
        <AmountInput id="tx-unit-price" v-model="form.unitPrice" />
        <p class="text-xs text-muted-foreground">{{ t('tx.field.unitPriceHint') }}</p>
      </div>
      <div v-if="form.kind === 'defi'" class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="tx-defi-action">{{ t('tx.field.action') }}</Label>
          <Select id="tx-defi-action" v-model="form.defiAction">
            <option v-for="entry in ['stake', 'unstake', 'claim', 'add_liquidity', 'remove_liquidity']" :key="entry" :value="entry">
              {{ t(`tx.defiAction.${entry}`) }}
            </option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-protocol">{{ t('tx.field.protocol') }}</Label>
          <Input id="tx-protocol" v-model="form.protocol" />
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-pool">{{ t('tx.field.pool') }}</Label>
          <Input id="tx-pool" v-model="form.poolId" />
        </div>
      </div>
      <div v-if="form.kind !== 'defi'" class="grid gap-1.5">
        <Label>{{ form.kind === 'topup' ? t('tx.field.fromWhom') : t('tx.field.toWhom') }}</Label>
        <AccountRefPicker v-model="form.counterpart" :stores="false" :empty-label="t('tx.side.unspecified')" />
      </div>
      <div v-if="form.kind === 'topup' && !props.template && !isProperty" class="grid gap-1.5">
        <p v-if="form.extra.length" class="text-xs text-muted-foreground">{{ t('tx.split.topupHint') }}</p>
        <FormField :error="check.error('extra')">
          <SplitRows v-model="form.extra" kind="topup" mode="direct" />
        </FormField>
      </div>
    </template>

    <!-- Transfer: one asset between two places -->
    <template v-else-if="form.kind === 'transfer'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <AssetInput v-model="form.asset" required :error="check.error('asset')" :label="t('form.asset')" />
        <FormField for="tx-amount" :label="t('form.amount')" required :error="check.error('amount')">
          <AmountInput id="tx-amount" v-model="form.amount" :placeholder="t('form.amountHint')" />
        </FormField>
      </div>
      <div class="grid items-end gap-3 @sm:grid-cols-[1fr_auto_1fr]">
        <FormField :label="t('tx.field.from')" required :error="check.error('from')">
          <AccountRefPicker v-model="form.from" />
        </FormField>
        <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
          <ArrowUpDown class="size-4" />
        </Button>
        <FormField :label="t('tx.field.to')" required :error="check.error('to')">
          <AccountRefPicker v-model="form.to" />
        </FormField>
      </div>
      <div v-if="debts.length" class="grid gap-1.5">
        <Label for="tx-debt">{{ t('debt.title') }}</Label>
        <Select id="tx-debt" v-model="debtValue">
          <option value="">{{ t('debt.none') }}</option>
          <option v-for="effect in debts" :key="effect" :value="effect">{{ t(`debt.${effect}`) }}</option>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('debt.hint') }}</p>
      </div>
    </template>

    <!-- Convert: give one asset, receive another — straight or through someone (P2P) -->
    <template v-else-if="form.kind === 'convert'">
      <ToggleGroup
        v-if="!props.template"
        type="single"
        variant="outline"
        size="sm"
        class="w-fit"
        :spacing="1"
        :model-value="form.convertMode"
        :aria-label="t('tx.p2p.mode')"
        data-tour="tx-convert-mode"
        @update:model-value="pickConvertMode"
      >
        <ToggleGroupItem value="direct">{{ t('tx.p2p.direct') }}</ToggleGroupItem>
        <ToggleGroupItem value="p2p">{{ t('tx.p2p.through') }}</ToggleGroupItem>
      </ToggleGroup>
      <p v-if="form.convertMode === 'p2p'" class="text-xs text-muted-foreground">{{ t('tx.p2p.hint') }}</p>

      <div class="grid gap-3 @sm:grid-cols-3">
        <FormField
          :label="form.convertMode === 'p2p' ? t('tx.p2p.from') : t('tx.field.give')"
          required
          :error="check.error('from')"
        >
          <AccountRefPicker v-model="form.from" :contacts="form.convertMode !== 'p2p'" :raw="form.convertMode !== 'p2p'" />
        </FormField>
        <AssetInput v-model="form.fromAsset" required :error="check.error('fromAsset')" :label="t('form.asset')" />
        <FormField for="tx-from-amount" :label="t('form.amount')" required :error="check.error('fromAmount')">
          <AmountInput id="tx-from-amount" v-model="form.fromAmount" />
        </FormField>
      </div>
      <FormField v-if="form.convertMode === 'p2p'" :label="t('tx.p2p.via')" required :error="check.error('via')">
        <AccountRefPicker v-model="form.via" :stores="false" />
      </FormField>
      <div class="flex justify-center">
        <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
          <ArrowUpDown class="size-4" />
        </Button>
      </div>
      <!-- What arrives, and the accounts it lands on: the counterparty may pay
           out to several of them; each row is an account and its amount. -->
      <div class="grid gap-3 @sm:grid-cols-[minmax(0,1fr)_14rem]">
        <div class="grid gap-1.5">
          <Label required>{{ form.convertMode === 'p2p' ? t('tx.p2p.receiveTo') : t('tx.field.receive') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('tx.split.receiveHint') }}</p>
        </div>
        <AssetInput v-model="form.toAsset" required :error="check.error('toAsset')" :label="t('tx.split.receiveAsset')" />
      </div>
      <div class="grid gap-2" data-tour="tx-split">
        <div class="grid items-start gap-2 @sm:grid-cols-[minmax(0,1fr)_10rem_2rem]">
          <FormField :error="check.error('to')">
            <AccountRefPicker v-model="form.to" :contacts="form.convertMode !== 'p2p'" :raw="form.convertMode !== 'p2p'" />
          </FormField>
          <FormField :error="check.error('toAmount')">
            <AmountInput id="tx-to-amount" v-model="form.toAmount" :placeholder="t('form.amount')" :aria-label="t('form.amount')" />
          </FormField>
        </div>
        <FormField v-if="!props.template" :error="check.error('extra')">
          <SplitRows v-model="form.extra" kind="convert" :mode="form.convertMode" />
        </FormField>
        <p v-if="form.extra.length && form.toAsset" class="text-xs text-muted-foreground">
          {{ t('tx.split.receivedTotal', { total: `${Number(receivedTotal.toFixed(8))} ${form.toAsset.symbol}` }) }}
        </p>
      </div>
      <p v-if="toAccount.moneyOnly.value && form.toAsset" class="text-xs text-muted-foreground">
        {{ t('tx.toCurrencyHint', { code: form.toAsset.symbol }) }}
      </p>
      <div class="grid gap-1.5">
        <Label for="tx-rate">{{ t('tx.field.rate') }}</Label>
        <AmountInput id="tx-rate" v-model="form.rate" :placeholder="impliedRate" />
        <p class="text-xs text-muted-foreground">{{ impliedRate || t('tx.field.rateHint') }}</p>
      </div>
    </template>

    <!-- Spot trade -->
    <template v-else-if="form.kind === 'trade_spot'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <AssetInput v-model="form.soldAsset" required :error="check.error('soldAsset')" :label="t('form.sellAsset')" />
        <FormField for="tx-sold" :label="t('form.sellQty')" required :error="check.error('soldQty')">
          <AmountInput id="tx-sold" v-model="form.soldQty" />
        </FormField>
        <AssetInput v-model="form.boughtAsset" required :error="check.error('boughtAsset')" :label="t('form.buyAsset')" />
        <FormField for="tx-bought" :label="t('form.buyQty')" required :error="check.error('boughtQty')">
          <AmountInput id="tx-bought" v-model="form.boughtQty" />
        </FormField>
      </div>
      <div class="grid gap-3 @sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="tx-liquidity">{{ t('tx.field.liquidity') }}</Label>
          <Select id="tx-liquidity" v-model="form.liquidity">
            <option value="">—</option>
            <option value="maker">maker</option>
            <option value="taker">taker</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-order">{{ t('tx.field.order') }}</Label>
          <Input id="tx-order" v-model="form.orderId" spellcheck="false" />
        </div>
      </div>
    </template>

    <!-- Futures -->
    <template v-else-if="form.kind === 'trade_futures'">
      <div class="grid gap-3 @sm:grid-cols-3">
        <FormField for="tx-symbol" :label="t('tx.field.contract')" required :error="check.error('symbol')">
          <Input id="tx-symbol" v-model="form.symbol" placeholder="BTCUSDT" spellcheck="false" />
        </FormField>
        <div class="grid gap-1.5">
          <Label for="tx-direction">{{ t('tx.field.direction') }}</Label>
          <Select id="tx-direction" v-model="form.direction">
            <option value="long">Long</option>
            <option value="short">Short</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-futures-action">{{ t('tx.field.action') }}</Label>
          <Select id="tx-futures-action" v-model="form.action">
            <option value="open">{{ t('tx.futures.open') }}</option>
            <option value="close">{{ t('tx.futures.close') }}</option>
          </Select>
        </div>
        <FormField for="tx-qty" :label="t('tx.field.quantity')" required :error="check.error('quantity')">
          <AmountInput id="tx-qty" v-model="form.quantity" />
        </FormField>
        <FormField for="tx-fill" :label="t('tx.field.price')" required :error="check.error('fillPrice')">
          <AmountInput id="tx-fill" v-model="form.fillPrice" />
        </FormField>
        <div class="grid gap-1.5">
          <Label for="tx-leverage">{{ t('tx.field.leverage') }}</Label>
          <AmountInput id="tx-leverage" v-model="form.leverage" />
        </div>
        <AssetInput v-model="form.settle" :label="t('tx.field.settle')" />
        <div class="grid gap-1.5">
          <Label for="tx-futures-category">{{ t('tx.field.futuresCategory') }}</Label>
          <Select id="tx-futures-category" v-model="form.futuresCategory">
            <option value="linear">linear</option>
            <option value="inverse">inverse</option>
          </Select>
        </div>
        <div v-if="form.action === 'close'" class="grid gap-1.5">
          <Label for="tx-pnl">{{ t('tx.field.closedPnl') }}</Label>
          <AmountInput id="tx-pnl" v-model="form.closedPnl" />
        </div>
      </div>
    </template>

    <!-- Selling property -->
    <template v-else-if="form.kind === 'asset_sell'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <FormField for="tx-sell-asset" :label="t('tx.field.property')" required :error="check.error('asset')">
          <Select id="tx-sell-asset" v-model="sellKey">
            <option value="" disabled>{{ t('tx.field.pickProperty') }}</option>
            <option v-for="asset in heldAssets" :key="asset.symbol" :value="asset.symbol">{{ asset.name || asset.symbol }}</option>
          </Select>
        </FormField>
        <FormField for="tx-sell-qty" :label="t('tx.field.units')" required :error="check.error('sellQty')">
          <AmountInput id="tx-sell-qty" v-model="form.sellQty" />
        </FormField>
        <FormField for="tx-sell-price" :label="t('tx.field.unitPrice')" required :error="check.error('sellPrice')">
          <AmountInput id="tx-sell-price" v-model="form.sellPrice" />
        </FormField>
        <AssetInput v-model="form.currency" required :error="check.error('currency')" :label="t('tx.field.currency')" />
      </div>
      <div class="grid gap-1.5">
        <Label for="tx-proceeds">{{ t('tx.field.proceedsTo') }}</Label>
        <Select id="tx-proceeds" v-model="form.proceedsTo">
          <option value="">{{ t('tx.side.unspecified') }}</option>
          <option v-for="node in proceedsStores" :key="node.store.id" :value="node.store.id">{{ node.path.join(' › ') }}</option>
        </Select>
      </div>
    </template>

    <!-- Recent Bybit fills fill a trade in -->
    <div v-if="showFills" class="grid gap-1.5">
      <div class="flex items-center justify-between gap-2">
        <Label>{{ t('tx.bybit.recent') }}</Label>
        <div class="flex items-center gap-2">
          <Select v-if="form.kind === 'trade_spot'" v-model="bybitCategory" class="h-8 w-28 text-xs">
            <option value="spot">spot</option>
          </Select>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            :disabled="lookup.executionsLoading"
            @click="lookup.loadExecutions(form.kind === 'trade_futures' ? 'linear' : 'spot')"
          >
            {{ t('actions.refresh') }}
          </Button>
        </div>
      </div>
      <p v-if="lookup.keyNotice" class="text-xs text-muted-foreground">{{ t(`tx.bybit.${lookup.keyNotice}`) }}</p>
      <div v-else-if="lookup.executions.length" class="grid max-h-40 gap-1 overflow-y-auto">
        <button
          v-for="exec in lookup.executions"
          :key="exec.exec_time + exec.exec_qty + exec.exec_price"
          type="button"
          class="flex items-center justify-between gap-2 rounded-md border border-border px-2 py-1.5 text-left text-xs transition-colors hover:bg-accent"
          @click="applyFill(exec)"
        >
          <span class="font-medium">{{ exec.symbol }}</span>
          <span :class="exec.side === 'Buy' ? 'text-emerald-600 dark:text-emerald-400' : 'text-destructive'">{{ exec.side }}</span>
          <span class="text-muted-foreground">{{ exec.exec_qty }} @ {{ exec.exec_price }}</span>
          <span class="text-muted-foreground">{{ formatDateTime(exec.exec_time) }}</span>
        </button>
      </div>
    </div>

    <div v-if="showChain" class="grid gap-3 @sm:grid-cols-2">
      <div class="grid gap-1.5">
        <Label for="tx-network">{{ t('form.network') }}</Label>
        <Select id="tx-network" v-model="form.network">
          <option value="">{{ t('form.noNetwork') }}</option>
          <option v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</option>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label for="tx-hash">{{ t('form.txHash') }}</Label>
        <Input id="tx-hash" v-model="form.txHash" spellcheck="false" placeholder="0x…" />
        <p v-if="lookupHint" class="text-xs text-muted-foreground">{{ lookupHint }}</p>
      </div>
    </div>

    <!-- No price field: the backend prices the operation at the rates of its moment
         (a price from an exchange fill or the calculator still travels in `form.price`). -->
    <div class="grid items-start gap-3 @sm:grid-cols-2">
      <div class="grid gap-1.5">
        <Label for="tx-fee">{{ t('form.fee') }}</Label>
        <AmountInput id="tx-fee" v-model="form.feeAmount" placeholder="0" />
      </div>
      <div v-if="!props.template" class="grid min-w-0 gap-1.5">
        <Label for="tx-date">{{ t('form.date') }}</Label>
        <Input id="tx-date" v-model="form.timestamp" type="datetime-local" class="min-w-0 px-2" />
        <p v-if="rateHint && form.kind !== 'asset_sell' && form.kind !== 'trade_futures'" class="text-xs text-muted-foreground">
          {{ rateHint }}
        </p>
      </div>
    </div>

    <FormField
      for="tx-categories"
      :label="t('form.categories')"
      :required="needsCategory && !props.template"
      :error="check.error('categories')"
      :hint="needsCategory ? t('tx.field.categoryHint') : undefined"
    >
      <CategorySelect id="tx-categories" v-model="form.categories" multiple :groups="categoryGroupsFor(form.kind)" />
    </FormField>

    <!-- Cashback: which card's purchases earned it, for the cashback-by-card report. -->
    <div v-if="form.kind === 'topup' && form.categories.includes('cashback')" class="grid gap-1.5">
      <Label for="tx-cashback-source">{{ t('loyalty.source') }}</Label>
      <Select id="tx-cashback-source" v-model="form.sourceStoreId">
        <option value="">{{ t('loyalty.noSource') }}</option>
        <option v-for="node in cashbackSources" :key="node.store.id" :value="node.store.id">{{ node.store.name }}</option>
      </Select>
    </div>

    <div class="grid gap-3 @sm:grid-cols-2">
      <div class="grid gap-1.5">
        <Label for="tx-contact">{{ t('tx.field.contact') }}</Label>
        <Select id="tx-contact" v-model="form.contactId">
          <option value="">{{ t('tx.side.unspecified') }}</option>
          <option v-for="contact in (contacts.data.value ?? []).filter((entry) => !entry.archived)" :key="contact.id" :value="contact.id">
            {{ contact.name }}
          </option>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label for="tx-comment">{{ t('form.comment') }}</Label>
        <Textarea id="tx-comment" v-model="form.comment" rows="1" :placeholder="t('form.commentHint')" />
      </div>
    </div>

    <p
      v-if="missing.length"
      class="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground"
      data-tour="tx-missing"
    >
      {{ t('tx.missing.title') }}: {{ missing.map((key) => t(`tx.missing.${key}`)).join(', ') }}
    </p>
    <p v-if="!missing.length && check.error('form')" class="text-xs text-destructive" role="alert">{{ check.error('form') }}</p>

    <TxPreviewCard
      v-if="debouncedDraft"
      :preview="preview.data.value"
      :loading="preview.isFetching.value"
    />
  </form>
</template>
