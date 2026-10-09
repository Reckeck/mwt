<script setup lang="ts">
import type { Ref } from 'vue'
import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { AssetInput } from '@/entities/asset'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { formatDateTime } from '@/shared/lib/format'
import {
  TX_COMMAND,
  TX_TAGS_ON_CHAIN,
  TX_TAGS_WITH_FEE,
  TX_TAGS_WITH_FLOW,
  type TxTag,
  readStoreKindKinds,
} from '@/shared/lib/transactions'
import { call, parseError } from '@/shared/api/ipc'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import {
  BANK_RAILS,
  DEFI_ACTIONS,
  INCOME_SOURCES,
  KIND_TEXT_FIELDS,
  REQUIRED_TEXT_FIELDS,
  TEXT_FIELD_LABELS,
  type KindTextField,
} from './add-transaction-form.constants'
import type { AddTransactionFormProps, TransactionPlan } from './add-transaction-form.types'

const props = defineProps<AddTransactionFormProps>()

/** Emitted after every successful write so the page can refresh its queries. */
const emit = defineEmits<{ saved: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const storeKind = computed(() => stores.data.value?.find((store) => store.id === props.storeId)?.kind ?? null)

/** Which kinds this wallet accepts. The binding lives in Rust; the form only asks. */
const kindsQuery = useQuery({
  queryKey: computed(() => ['transaction-kinds', storeKind.value]),
  queryFn: async () =>
    readStoreKindKinds(await call<TransactionKinds>('transaction_kinds_for_store_kind', { kind: storeKind.value })),
  enabled: computed(() => storeKind.value !== null),
})

const offeredKinds = computed<TxTag[]>(() => {
  const kinds = kindsQuery.data.value
  return kinds ? [...kinds.specific, ...kinds.common] : []
})

const kind = ref<TxTag>('income')

// Land on the wallet's default kind once known, and leave a kind it does not offer.
watch(
  () => kindsQuery.data.value,
  (kinds) => {
    if (kinds && !offeredKinds.value.includes(kind.value)) kind.value = kinds.default
  },
  { immediate: true },
)

const isTrade = computed(() => kind.value === 'exchange_trade')
const isTransfer = computed(() => kind.value === 'transfer')
const hasFlow = computed(() => TX_TAGS_WITH_FLOW.has(kind.value))
const hasFee = computed(() => TX_TAGS_WITH_FEE.has(kind.value))
const onChain = computed(() => TX_TAGS_ON_CHAIN.has(kind.value))
const textFields = computed(() => KIND_TEXT_FIELDS[kind.value] ?? [])

const asset = ref<AssetRef | null>(null)
const sellAsset = ref<AssetRef | null>(null)
const buyAsset = ref<AssetRef | null>(null)
const toStoreId = ref('')
const amount = ref('')
const boughtQty = ref('')
const priceUsd = ref('')
const fee = ref('')
const selectedCategories = ref<string[]>([])
const categoryDraft = ref('')
const comment = ref('')
const network = ref<Chain | ''>('')
const txHash = ref('')
const timestamp = ref('')
const flow = ref<Flow>('in')
const rail = ref<BankRail>('internal')
const incomeSource = ref<IncomeSource>('other')
const defiAction = ref<DefiAction>('stake')
/** Kind-specific free text (payee, IBAN, merchant…), keyed by input field name. */
const extra = reactive<Partial<Record<KindTextField, string>>>({})

const lookup = useTradeLookupStore()

const bybitCategory = ref<Category>('spot')

function reloadExecutions() {
  void lookup.loadExecutions(bybitCategory.value)
}

// Load once when the form opens, so the list is ready without an extra click.
watch(
  () => lookup.hasKey,
  (configured) => {
    if (configured) reloadExecutions()
  },
  { immediate: true },
)

/**
 * Only ever fills fields the user has not touched. A lookup is an assistant,
 * not an override: anything already typed wins.
 */
function fillIfEmpty(target: Ref<string>, value: string | null | undefined) {
  if (!value?.trim()) return false
  if (target.value.trim()) return false
  target.value = value.trim()
  return true
}

/** Translate a backend hint into a form field, leaving typed values alone. */
function applyHint(hint: TxFieldHint): boolean {
  switch (hint.field) {
    case 'qtyBought':
      return fillIfEmpty(isTrade.value ? boughtQty : amount, hint.value)
    case 'qtySold':
      return fillIfEmpty(amount, hint.value)
    case 'price':
      return fillIfEmpty(priceUsd, hint.value)
    case 'fee':
      return fillIfEmpty(fee, hint.value)
    case 'occurredAt':
      return fillIfEmpty(timestamp, hint.value)
    default:
      return false
  }
}

/** Auto-inspect once the hash stops changing. */
let hashTimer: ReturnType<typeof setTimeout> | undefined
watch(txHash, (value) => {
  if (hashTimer) clearTimeout(hashTimer)
  const hash = value.trim()
  if (!hash) {
    lookup.reset()
    return
  }
  hashTimer = setTimeout(() => {
    void lookup.inspectTx(hash, network.value || undefined)
  }, 600)
})

/** Quiet, non-blocking reason why nothing was filled in. */
const lookupHint = computed(() => {
  if (lookup.loading) return t('tx.lookup.checking')
  if (lookup.notice) return t(`tx.lookup.${lookup.notice}`)
  return null
})

// Apply whatever the backend managed to suggest, field by field.
watch(
  () => lookup.hints,
  (hints) => {
    for (const hint of hints) applyHint(hint)
  },
  { deep: true },
)

/** Apply a Bybit fill to the form as a trade. */
async function applyBybitExecution(exec: BybitExecution) {
  const fill = exec.prefill
  kind.value = 'exchange_trade'
  fillIfEmpty(amount, fill.qty_sold)
  fillIfEmpty(boughtQty, fill.qty_bought)
  fillIfEmpty(priceUsd, fill.price)
  fillIfEmpty(fee, fill.fee)
  fillIfEmpty(comment, fill.comment)
  if (!timestamp.value.trim() && fill.occurred_at) timestamp.value = fill.occurred_at

  // Resolve the two sides by symbol; leave them untouched if the search fails.
  const resolve = async (symbol: string | null | undefined): Promise<AssetRef | null> => {
    if (!symbol) return null
    const hits = await call<AssetSearchHit[]>('search_assets', { query: symbol })
    const hit = hits.find((item) => item.symbol.toUpperCase() === symbol.toUpperCase()) ?? hits[0]
    if (!hit) return null
    const sym = hit.symbol.toUpperCase()
    return {
      symbol: sym,
      name: hit.name || sym,
      coingecko_id: hit.coingecko_id,
      chain: hit.chain ?? null,
      contract: hit.contract ?? null,
      // Same convention as `AssetInput`: display-only quote marker.
      quote: sym,
    }
  }
  const [sold, bought] = await Promise.all([resolve(fill.asset_sold), resolve(fill.asset_bought)])
  if (!sellAsset.value && sold) sellAsset.value = sold
  if (!buyAsset.value && bought) buyAsset.value = bought
}

const categories = useQuery({
  queryKey: ['categories'],
  queryFn: () => call<string[]>('list_categories'),
})

/**
 * Tags offered by the picker: the vault-wide vocabulary, plus whatever this
 * transaction already carries (an edited transaction may hold a tag the
 * vocabulary has since lost).
 */
const categoryOptions = computed(() => {
  const known = categories.data.value ?? []
  return [...new Set([...selectedCategories.value, ...known])].sort()
})

/** Same canonical form the backend applies — keep the two in step. */
function normalizeCategory(raw: string): string {
  return raw.trim().toLowerCase()
}

function isCategoryPicked(category: string): boolean {
  return selectedCategories.value.includes(category)
}

function toggleCategory(category: string) {
  selectedCategories.value = isCategoryPicked(category)
    ? selectedCategories.value.filter((entry) => entry !== category)
    : [...selectedCategories.value, category]
}

/** Add a brand-new tag from the free-text field. */
function addCategory() {
  const normalized = normalizeCategory(categoryDraft.value)
  if (!normalized || isCategoryPicked(normalized)) {
    categoryDraft.value = ''
    return
  }
  selectedCategories.value = [...selectedCategories.value, normalized]
  categoryDraft.value = ''
}

/** Other stores are the only valid transfer destinations. */
const otherStores = computed(() =>
  (stores.data.value ?? []).filter((store) => store.id !== props.storeId && !store.archived),
)

watch(
  otherStores,
  (list) => {
    if (!toStoreId.value && list.length) toStoreId.value = list[0].id
  },
  { immediate: true },
)

/** Keep the network dropdown in sync with the picked asset. */
watch([asset, sellAsset, buyAsset], () => {
  const picked = [asset.value, sellAsset.value, buyAsset.value].find((ref) => ref?.chain && ref.chain !== 'fiat')
  if (picked?.chain) network.value = picked.chain
})

function reset() {
  amount.value = ''
  boughtQty.value = ''
  priceUsd.value = ''
  fee.value = ''
  selectedCategories.value = []
  categoryDraft.value = ''
  comment.value = ''
  txHash.value = ''
  for (const key of Object.keys(extra) as KindTextField[]) delete extra[key]
  lookup.reset()
}

function invalidate() {
  void queryClient.invalidateQueries({ queryKey: ['transactions'] })
  void queryClient.invalidateQueries({ queryKey: ['store', props.storeId] })
  void queryClient.invalidateQueries({ queryKey: ['store-history', props.storeId] })
  void queryClient.invalidateQueries({ queryKey: ['store-pnl', props.storeId] })
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
  void queryClient.invalidateQueries({ queryKey: ['stores'] })
  void queryClient.invalidateQueries({ queryKey: ['history'] })
  emit('saved')
}

const optional = (value: string | undefined): string | null => value?.trim() || null

function identOf(ref: AssetRef): AssetIdent {
  return { symbol: ref.symbol, name: ref.name, coingecko_id: ref.coingecko_id, chain: ref.chain, contract: ref.contract }
}

/** The kind-agnostic part every input carries. */
function common(): TxInputCommon {
  return {
    amount: amount.value.trim(),
    comment: comment.value.trim(),
    categories: [...selectedCategories.value],
    timestamp: timestamp.value ? new Date(timestamp.value).toISOString() : null,
    price_usd: priceUsd.value.trim() || null,
    fx_override: null,
  }
}

/** Build the input DTO of the selected kind; each kind sends only its own fields. */
function buildInput(): Record<string, unknown> {
  const feeFields = { fee: fee.value.trim() || null, fee_asset: null }
  const chainFields = { network: network.value || null, tx_hash: txHash.value.trim() || null }

  if (kind.value === 'exchange_trade') {
    if (!sellAsset.value || !buyAsset.value) throw new Error('asset')
    const input: TradeInput = {
      store_id: props.storeId,
      sold: identOf(sellAsset.value),
      sold_qty: amount.value.trim(),
      bought: identOf(buyAsset.value),
      bought_qty: boughtQty.value.trim(),
      common: common(),
      ...feeFields,
      order_id: optional(extra.order_id),
      market: optional(extra.market),
      liquidity: optional(extra.liquidity),
    }
    return input
  }

  if (!asset.value) throw new Error('asset')
  const base = { store_id: props.storeId, asset: identOf(asset.value), common: common() }

  switch (kind.value) {
    case 'transfer': {
      const input: TransferInput = {
        from_store_id: props.storeId,
        to_store_id: toStoreId.value,
        asset: base.asset,
        common: base.common,
        ...feeFields,
        ...chainFields,
      }
      return input
    }
    case 'income':
      return { ...base, source: incomeSource.value } satisfies IncomeInput
    case 'expense':
      return base satisfies ExpenseInput
    case 'payment':
      return { ...base, payee: extra.payee?.trim() ?? '', receipt_ref: optional(extra.receipt_ref) } satisfies PaymentInput
    case 'fee':
      return { ...base, related_id: null } satisfies FeeInput
    case 'bank_transfer':
      return {
        ...base,
        flow: flow.value,
        rail: rail.value,
        counterparty_name: extra.counterparty_name?.trim() ?? '',
        counterparty_account: optional(extra.counterparty_account),
        reference: optional(extra.reference),
        ...feeFields,
      } satisfies BankTransferInput
    case 'card_payment':
      return {
        ...base,
        card_last4: optional(extra.card_last4),
        merchant: optional(extra.merchant),
        mcc: optional(extra.mcc),
        ...feeFields,
      } satisfies CardPaymentInput
    case 'wallet_transfer':
      return {
        ...base,
        flow: flow.value,
        ...chainFields,
        from_address: optional(extra.from_address),
        to_address: optional(extra.to_address),
        memo: optional(extra.memo),
        ...feeFields,
      } satisfies WalletTransferInput
    case 'defi_action':
      return {
        ...base,
        flow: flow.value,
        action: defiAction.value,
        ...chainFields,
        protocol: optional(extra.protocol),
        pool_id: optional(extra.pool_id),
        ...feeFields,
      } satisfies DefiActionInput
    case 'cash_flow':
      return {
        ...base,
        flow: flow.value,
        location: optional(extra.location),
        counterparty: optional(extra.counterparty),
      } satisfies CashFlowInput
    default:
      throw new Error(`no input for ${kind.value}`)
  }
}

const save = useMutation({
  mutationFn: () => call<Transaction>(TX_COMMAND[kind.value], { input: buildInput() }),
  onSuccess: () => {
    toast.success(t('form.saved'))
    reset()
    invalidate()
  },
  onError: (err) => {
    const parsed = parseError(err)
    const known = t(`errors.${parsed.code}`)
    const message =
      parsed.code === 'UNKNOWN' ? t('form.assetRequired') : known === `errors.${parsed.code}` ? parsed.message : known
    toast.error(t('form.saveFailed'), { description: message })
  },
})

/** Fill the form from a calculator plan. */
function prefill(plan: TransactionPlan) {
  const tradeOffered = offeredKinds.value.includes('exchange_trade')
  if (plan.mode === 'converter' && plan.from && plan.to && tradeOffered) {
    kind.value = 'exchange_trade'
    sellAsset.value = plan.from
    buyAsset.value = plan.to
  } else if (plan.from) {
    kind.value = 'income'
    asset.value = plan.from
  }
  amount.value = plan.amount
  // The calculator's price is the USD price of what is being sold/received.
  priceUsd.value = plan.price
  comment.value = ''
}

/** A calculator plan handed in by the page, applied once the wallet's kinds are known. */
let appliedPlan: TransactionPlan | null = null
watch(
  () => [props.plan, kindsQuery.data.value] as const,
  ([plan, kinds]) => {
    if (!plan || !kinds || plan === appliedPlan) return
    appliedPlan = plan
    prefill(plan)
  },
  { immediate: true },
)

/** The kind always has a value: pressing the active one again is ignored. */
function pickKind(value: unknown) {
  if (typeof value === 'string' && value) kind.value = value as TxTag
}


const valid = computed(() => {
  if (!amount.value.trim()) return false
  if (isTrade.value) return Boolean(sellAsset.value && buyAsset.value && boughtQty.value.trim())
  if (!asset.value) return false
  if (isTransfer.value) return Boolean(toStoreId.value)
  return REQUIRED_TEXT_FIELDS.every((field) => !textFields.value.includes(field) || extra[field]?.trim())
})

/** The dialog owns the submit button, so it reads these. */
defineExpose({ valid, pending: computed(() => save.isPending.value) })
</script>

<template>
  <form id="transaction-form" class="flex flex-col gap-4" @submit.prevent="save.mutate()">
    <ToggleGroup
      type="single"
      variant="outline"
      size="sm"
      class="flex-wrap"
      :spacing="1"
      :model-value="kind"
      :aria-label="t('form.kind')"
      @update:model-value="pickKind"
    >
      <ToggleGroupItem v-for="entry in offeredKinds" :key="entry" :value="entry">
        {{ t(`tx.kind.${entry}`) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <div v-if="isTrade" class="grid gap-3 @sm:grid-cols-2">
      <AssetInput v-model="sellAsset" :label="t('form.sellAsset')" />
      <AssetInput v-model="buyAsset" :label="t('form.buyAsset')" />
    </div>
    <div v-else class="grid gap-3 @sm:grid-cols-2">
      <AssetInput v-model="asset" :label="t('form.asset')" />
      <div v-if="isTransfer" class="grid gap-1.5">
        <Label>{{ t('form.toStore') }}</Label>
        <Select v-model="toStoreId">
          <option v-for="store in otherStores" :key="store.id" :value="store.id">{{ store.name }}</option>
        </Select>
      </div>
    </div>

    <div class="grid gap-3 @sm:grid-cols-3">
      <div class="grid gap-1.5">
        <Label>{{ t(isTrade ? 'form.sellQty' : 'form.amount') }}</Label>
        <Input v-model="amount" inputmode="decimal" :placeholder="t('form.amountHint')" required />
      </div>
      <div v-if="isTrade" class="grid gap-1.5">
        <Label>{{ t('form.buyQty') }}</Label>
        <Input v-model="boughtQty" inputmode="decimal" :placeholder="t('form.buyQtyHint')" required />
      </div>
      <div class="grid gap-1.5">
        <Label>{{ t('form.priceUsd') }}</Label>
        <Input v-model="priceUsd" inputmode="decimal" :placeholder="t('form.priceHint')" />
      </div>
      <div v-if="hasFee" class="grid gap-1.5">
        <Label>{{ t('form.fee') }}</Label>
        <Input v-model="fee" inputmode="decimal" placeholder="0" />
      </div>
    </div>

    <div v-if="hasFlow || kind === 'income' || kind === 'bank_transfer' || kind === 'defi_action'" class="grid gap-3 @sm:grid-cols-3">
      <div v-if="hasFlow" class="grid gap-1.5">
        <Label>{{ t('form.direction') }}</Label>
        <Select v-model="flow">
          <option value="in">{{ t('form.flow.in') }}</option>
          <option value="out">{{ t('form.flow.out') }}</option>
        </Select>
      </div>
      <div v-if="kind === 'income'" class="grid gap-1.5">
        <Label>{{ t('tx.detail.source') }}</Label>
        <Select v-model="incomeSource">
          <option v-for="entry in INCOME_SOURCES" :key="entry" :value="entry">{{ t(`form.source.${entry}`) }}</option>
        </Select>
      </div>
      <div v-if="kind === 'bank_transfer'" class="grid gap-1.5">
        <Label>{{ t('tx.detail.rail') }}</Label>
        <Select v-model="rail">
          <option v-for="entry in BANK_RAILS" :key="entry" :value="entry">{{ t(`form.rail.${entry}`) }}</option>
        </Select>
      </div>
      <div v-if="kind === 'defi_action'" class="grid gap-1.5">
        <Label>{{ t('tx.detail.action') }}</Label>
        <Select v-model="defiAction">
          <option v-for="entry in DEFI_ACTIONS" :key="entry" :value="entry">{{ t(`form.defiAction.${entry}`) }}</option>
        </Select>
      </div>
    </div>

    <div v-if="textFields.length" class="grid gap-3 @sm:grid-cols-3">
      <div v-for="field in textFields" :key="field" class="grid gap-1.5">
        <Label>{{ t(TEXT_FIELD_LABELS[field]) }}</Label>
        <Input v-model="extra[field]" spellcheck="false" :required="REQUIRED_TEXT_FIELDS.includes(field)" />
      </div>
    </div>

    <div v-if="lookup.hasKey" class="grid gap-1.5">
      <div class="flex items-center justify-between gap-2">
        <Label>{{ t('tx.bybit.recent') }}</Label>
        <div class="flex items-center gap-2">
          <Select v-model="bybitCategory" class="h-8 w-28 text-xs">
            <option value="spot">spot</option>
            <option value="linear">linear</option>
            <option value="inverse">inverse</option>
          </Select>
          <Button variant="ghost" size="sm" :disabled="lookup.executionsLoading" @click="reloadExecutions">
            {{ t('actions.refresh') }}
          </Button>
        </div>
      </div>
      <p v-if="lookup.keyNotice" class="text-xs text-muted-foreground">
        {{ t(`tx.bybit.${lookup.keyNotice}`) }}
      </p>
      <div v-else-if="lookup.executions.length" class="grid gap-1">
        <button
          v-for="(exec, i) in lookup.executions"
          :key="exec.exec_time + i"
          type="button"
          class="flex items-center justify-between gap-2 rounded-md border border-border px-2 py-1.5 text-left text-xs transition-colors hover:bg-accent"
          @click="applyBybitExecution(exec)"
        >
          <span class="font-medium">{{ exec.symbol }}</span>
          <span :class="exec.side === 'Buy' ? 'text-emerald-600' : 'text-rose-600'">{{ exec.side }}</span>
          <span class="text-muted-foreground">{{ exec.exec_qty }} @ {{ exec.exec_price }}</span>
          <span class="text-muted-foreground">{{ formatDateTime(exec.exec_time) }}</span>
        </button>
      </div>
    </div>

    <div class="grid gap-1.5">
      <Label>{{ t('form.categories') }}</Label>
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="entry in categoryOptions"
          :key="entry"
          type="button"
          class="rounded-full border px-2.5 py-0.5 text-xs font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
          :class="
            isCategoryPicked(entry)
              ? 'border-transparent bg-primary text-primary-foreground'
              : 'border-border text-muted-foreground hover:bg-accent hover:text-accent-foreground'
          "
          :aria-pressed="isCategoryPicked(entry)"
          @click="toggleCategory(entry)"
        >
          {{ entry }}
        </button>
        <span v-if="!categoryOptions.length" class="text-xs text-muted-foreground">
          {{ t('form.noCategoriesYet') }}
        </span>
      </div>
      <div class="flex items-center gap-2">
        <Input
          v-model="categoryDraft"
          :placeholder="t('form.categoryHint')"
          @keyup.enter="addCategory"
        />
        <Button type="button" variant="outline" size="sm" @click="addCategory">
          {{ t('form.addCategory') }}
        </Button>
      </div>
    </div>

    <div class="grid gap-3 @sm:grid-cols-2 @lg:grid-cols-4">
      <div v-if="onChain" class="grid gap-1.5">
        <Label>{{ t('form.network') }}</Label>
        <Select v-model="network">
          <option value="">{{ t('form.noNetwork') }}</option>
          <option v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</option>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label>{{ t('form.date') }}</Label>
        <Input v-model="timestamp" type="datetime-local" />
      </div>
      <div v-if="onChain" class="grid gap-1.5">
        <Label>{{ t('form.txHash') }}</Label>
        <Input v-model="txHash" spellcheck="false" placeholder="0x…" />
        <p v-if="lookupHint" class="text-xs text-muted-foreground">{{ lookupHint }}</p>
        <a
          v-else-if="lookup.explorerUrl"
          :href="lookup.explorerUrl"
          target="_blank"
          rel="noreferrer noopener"
          class="text-xs text-primary underline-offset-4 hover:underline"
        >
          {{ t('tx.lookup.openExplorer') }}
        </a>
      </div>
    </div>

    <div class="grid gap-1.5">
      <Label>{{ t('form.comment') }}</Label>
      <Textarea v-model="comment" :placeholder="t('form.commentHint')" required />
    </div>
  </form>
</template>
