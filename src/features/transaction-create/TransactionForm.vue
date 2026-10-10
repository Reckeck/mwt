<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ArrowUpDown } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { AssetInput } from '@/entities/asset'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
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
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import { AccountRefPicker } from './ui/account-ref-picker'
import { CategoryPicker } from './ui/category-picker'
import type { TransactionFormProps } from './transaction-form.types'
import {
  categoryGroupsFor,
  debtChoices,
  emptyForm,
  formFromTransaction,
  formToInput,
  isFormComplete,
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
const contextStore = computed(() => nodes.value.find((node) => node.store.id === props.storeId)?.store ?? null)
const storeKind = computed(() => contextStore.value?.kind ?? null)
const isProperty = computed(() => storeKind.value === 'asset')
const isCrypto = computed(() => ['exchange', 'hot_wallet', 'cold_wallet'].includes(storeKind.value ?? ''))

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
      form.to = props.counterpart
    } else if (!offered.value.includes(form.kind)) form.kind = data.default
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
    if (editing.value || form.asset || !kind || kind === 'asset') return
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

// A conversion defaults to staying on this wallet (a bank's currency exchange).
watch(
  () => form.kind,
  (kind) => {
    if (kind === 'convert' && !form.to) form.to = { store: props.storeId }
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
  enabled: computed(() => !props.template && Boolean(priced.value) && !form.priceUsd.trim()),
})
/** Which day's rate an empty price field stands for — or that there is none yet. */
const rateHint = computed(() => {
  const data = quote.data.value
  // A dollar is always worth a dollar; saying so is noise.
  if (!priced.value || priced.value.symbol.toUpperCase() === 'USD' || form.priceUsd.trim() || !data) return ''
  if (!data.ts) return t('tx.rate.none')
  const price = Number(data.price_usd)
  return t('tx.rate.at', {
    date: formatDateTime(data.ts),
    price: formatMoney(price, { decimals: price < 1 ? 6 : 2, currency: 'usd' }),
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
const loadedPrice = form.priceUsd
watch(
  () => form.timestamp,
  () => {
    if (editing.value && form.priceUsd === loadedPrice) form.priceUsd = ''
  },
)

/* --- assistants: tx-hash inspection and recent Bybit fills (never override typed values) --- */
/** Text fields an assistant may fill. */
type FillableField = 'feeAmount' | 'priceUsd' | 'amount' | 'comment' | 'soldQty' | 'boughtQty' | 'symbol' | 'quantity' | 'fillPrice'

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
      if (hint.field === 'price') fillIfEmpty('priceUsd', hint.value)
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
  fillIfEmpty('priceUsd', fill.price)
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
    form.priceUsd = plan.price
  },
  { immediate: true },
)

/* --- save --- */
const STALE_KEYS = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances']]

const save = useMutation({
  mutationFn: () => {
    const input = formToInput(form)
    if (!input) throw { code: 'INVALID_INPUT', message: t('form.assetRequired') }
    return props.transaction
      ? call<Transaction>('update_transaction', { input: { id: txId(props.transaction), transaction: input } })
      : call<Transaction>('add_transaction', { input })
  },
  onSuccess: (tx) => {
    toast.success(editing.value ? t('tx.updated') : t('form.saved'))
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

function submit() {
  if (props.template) emit('submit')
  else save.mutate()
}

/** The dialog owns the submit button, so it reads these; a template's parent reads `input`. */
defineExpose({
  valid,
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
    @submit.prevent="submit"
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

    <!-- Topup · Payment · DeFi: one asset on this wallet -->
    <template v-if="form.kind === 'topup' || form.kind === 'payment' || form.kind === 'defi'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <div v-if="isProperty && form.kind === 'topup'" class="grid gap-1.5">
          <Label for="tx-property">{{ t('tx.field.property') }}</Label>
          <Input id="tx-property" v-model="propertyName" :placeholder="t('tx.field.propertyHint')" />
        </div>
        <AssetInput v-else v-model="form.asset" :label="t('form.asset')" />
        <div class="grid gap-1.5">
          <Label for="tx-amount">{{ isProperty ? t('tx.field.units') : t('form.amount') }}</Label>
          <Input id="tx-amount" v-model="form.amount" inputmode="decimal" :placeholder="t('form.amountHint')" />
        </div>
      </div>
      <p
        v-if="overrunText"
        class="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
      >
        {{ overrunText }}
      </p>
      <div v-if="isProperty && form.kind === 'topup'" class="grid gap-1.5">
        <Label for="tx-unit-price">{{ t('tx.field.unitPrice') }}</Label>
        <Input id="tx-unit-price" v-model="form.unitPrice" inputmode="decimal" />
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
    </template>

    <!-- Transfer: one asset between two places -->
    <template v-else-if="form.kind === 'transfer'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <AssetInput v-model="form.asset" :label="t('form.asset')" />
        <div class="grid gap-1.5">
          <Label for="tx-amount">{{ t('form.amount') }}</Label>
          <Input id="tx-amount" v-model="form.amount" inputmode="decimal" :placeholder="t('form.amountHint')" />
        </div>
      </div>
      <div class="grid items-end gap-3 @sm:grid-cols-[1fr_auto_1fr]">
        <div class="grid gap-1.5">
          <Label>{{ t('tx.field.from') }}</Label>
          <AccountRefPicker v-model="form.from" />
        </div>
        <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
          <ArrowUpDown class="size-4" />
        </Button>
        <div class="grid gap-1.5">
          <Label>{{ t('tx.field.to') }}</Label>
          <AccountRefPicker v-model="form.to" />
        </div>
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

    <!-- Convert: give one asset, receive another -->
    <template v-else-if="form.kind === 'convert'">
      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label>{{ t('tx.field.give') }}</Label>
          <AccountRefPicker v-model="form.from" />
        </div>
        <AssetInput v-model="form.fromAsset" :label="t('form.asset')" />
        <div class="grid gap-1.5">
          <Label for="tx-from-amount">{{ t('form.amount') }}</Label>
          <Input id="tx-from-amount" v-model="form.fromAmount" inputmode="decimal" />
        </div>
      </div>
      <div class="flex justify-center">
        <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
          <ArrowUpDown class="size-4" />
        </Button>
      </div>
      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label>{{ t('tx.field.receive') }}</Label>
          <AccountRefPicker v-model="form.to" />
        </div>
        <AssetInput v-model="form.toAsset" :label="t('form.asset')" />
        <div class="grid gap-1.5">
          <Label for="tx-to-amount">{{ t('form.amount') }}</Label>
          <Input id="tx-to-amount" v-model="form.toAmount" inputmode="decimal" />
        </div>
      </div>
      <div class="grid gap-1.5">
        <Label for="tx-rate">{{ t('tx.field.rate') }}</Label>
        <Input id="tx-rate" v-model="form.rate" inputmode="decimal" />
        <p class="text-xs text-muted-foreground">{{ t('tx.field.rateHint') }}</p>
      </div>
    </template>

    <!-- Spot trade -->
    <template v-else-if="form.kind === 'trade_spot'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <AssetInput v-model="form.soldAsset" :label="t('form.sellAsset')" />
        <div class="grid gap-1.5">
          <Label for="tx-sold">{{ t('form.sellQty') }}</Label>
          <Input id="tx-sold" v-model="form.soldQty" inputmode="decimal" />
        </div>
        <AssetInput v-model="form.boughtAsset" :label="t('form.buyAsset')" />
        <div class="grid gap-1.5">
          <Label for="tx-bought">{{ t('form.buyQty') }}</Label>
          <Input id="tx-bought" v-model="form.boughtQty" inputmode="decimal" />
        </div>
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
        <div class="grid gap-1.5">
          <Label for="tx-symbol">{{ t('tx.field.contract') }}</Label>
          <Input id="tx-symbol" v-model="form.symbol" placeholder="BTCUSDT" spellcheck="false" />
        </div>
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
        <div class="grid gap-1.5">
          <Label for="tx-qty">{{ t('tx.field.quantity') }}</Label>
          <Input id="tx-qty" v-model="form.quantity" inputmode="decimal" />
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-fill">{{ t('tx.field.price') }}</Label>
          <Input id="tx-fill" v-model="form.fillPrice" inputmode="decimal" />
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-leverage">{{ t('tx.field.leverage') }}</Label>
          <Input id="tx-leverage" v-model="form.leverage" inputmode="decimal" />
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
          <Input id="tx-pnl" v-model="form.closedPnl" inputmode="decimal" />
        </div>
      </div>
    </template>

    <!-- Selling property -->
    <template v-else-if="form.kind === 'asset_sell'">
      <div class="grid gap-3 @sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="tx-sell-asset">{{ t('tx.field.property') }}</Label>
          <Select id="tx-sell-asset" v-model="sellKey">
            <option value="" disabled>{{ t('tx.field.pickProperty') }}</option>
            <option v-for="asset in heldAssets" :key="asset.symbol" :value="asset.symbol">{{ asset.name || asset.symbol }}</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-sell-qty">{{ t('tx.field.units') }}</Label>
          <Input id="tx-sell-qty" v-model="form.sellQty" inputmode="decimal" />
        </div>
        <div class="grid gap-1.5">
          <Label for="tx-sell-price">{{ t('tx.field.unitPrice') }}</Label>
          <Input id="tx-sell-price" v-model="form.sellPrice" inputmode="decimal" />
        </div>
        <AssetInput v-model="form.currency" :label="t('tx.field.currency')" />
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

    <div class="grid items-start gap-3 @sm:grid-cols-3">
      <div class="grid gap-1.5">
        <Label for="tx-fee">{{ t('form.fee') }}</Label>
        <Input id="tx-fee" v-model="form.feeAmount" inputmode="decimal" placeholder="0" />
      </div>
      <div v-if="!props.template && form.kind !== 'asset_sell' && form.kind !== 'trade_futures'" class="grid gap-1.5">
        <Label for="tx-price">{{ t('form.priceUsd') }}</Label>
        <Input id="tx-price" v-model="form.priceUsd" inputmode="decimal" :placeholder="t('form.priceHint')" />
        <p v-if="rateHint" class="text-xs text-muted-foreground">{{ rateHint }}</p>
      </div>
      <div v-if="!props.template" class="grid gap-1.5">
        <Label for="tx-date">{{ t('form.date') }}</Label>
        <Input id="tx-date" v-model="form.timestamp" type="datetime-local" />
      </div>
    </div>

    <div class="grid gap-1.5">
      <Label>{{ t('form.categories') }}</Label>
      <CategoryPicker v-model="form.categories" :groups="categoryGroupsFor(form.kind)" />
      <p v-if="form.kind === 'topup' || form.kind === 'payment'" class="text-xs text-muted-foreground">
        {{ t('tx.field.categoryHint') }}
      </p>
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
  </form>
</template>
