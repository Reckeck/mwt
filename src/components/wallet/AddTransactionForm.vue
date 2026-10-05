<script setup lang="ts">
import type { Ref } from 'vue'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import AssetInput from '@/components/AssetInput.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import Textarea from '@/components/ui/textarea.vue'
import { CHAIN_LABELS, CHAINS } from '@/lib/chains'
import { formatDateTime } from '@/lib/format'
import { call, parseError } from '@/lib/ipc'
import { useTradeLookupStore } from '@/stores/tradeLookup'

const props = defineProps<{ storeId: string; disabled?: boolean }>()

/** Emitted after every successful write so the page can refresh its queries. */
const emit = defineEmits<{ saved: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

type FormMode = 'income' | 'expense' | 'transfer' | 'trade'

const mode = ref<FormMode>('income')

const asset = ref<AssetRef | null>(null)
const sellAsset = ref<AssetRef | null>(null)
const buyAsset = ref<AssetRef | null>(null)
const toStoreId = ref('')
const amount = ref('')
const priceUsd = ref('')
const fee = ref('')
const category = ref('')
const comment = ref('')
const network = ref<Chain | ''>('')
const txHash = ref('')
const timestamp = ref('')

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
function fillIfEmpty(target: Ref<string>, value: string | undefined) {
  if (!value?.trim()) return false
  if (target.value.trim()) return false
  target.value = value.trim()
  return true
}

/** Translate a backend hint into a form field, leaving typed values alone. */
function applyHint(hint: TxFieldHint): boolean {
  switch (hint.field) {
    case 'qtyBought':
    case 'qtySold':
      return fillIfEmpty(amount, hint.value)
    case 'price':
      return fillIfEmpty(priceUsd, hint.value)
    case 'fee':
      return fillIfEmpty(fee, hint.value)
    case 'occurredAt':
      return fillIfEmpty(timestamp, hint.value)
    case 'status':
    case 'assetBought':
    case 'assetSold':
    case 'feeAsset':
      return false
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
  mode.value = 'trade'
  fillIfEmpty(amount, fill.side === 'buy' ? fill.qty_bought ?? '' : fill.qty_sold ?? '')
  fillIfEmpty(priceUsd, fill.price ?? '')
  fillIfEmpty(fee, fill.fee ?? '')
  fillIfEmpty(comment, fill.comment ?? '')
  if (!timestamp.value.trim() && fill.occurred_at) timestamp.value = fill.occurred_at

  // Resolve the two legs by symbol; leave them untouched if the search fails.
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

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const categories = useQuery({
  queryKey: ['categories'],
  queryFn: () => call<string[]>('list_categories'),
})

const isTransfer = computed(() => mode.value === 'transfer')
const isTrade = computed(() => mode.value === 'trade')

/** Other stores are the only valid transfer destinations. */
const otherStores = computed(() =>
  (stores.data.value ?? []).filter((store) => store.id !== props.storeId && !store.archived),
)

watch(otherStores, (list) => {
  if (!toStoreId.value && list.length) toStoreId.value = list[0].id
}, { immediate: true })

/** Keep the network dropdown in sync with the picked asset. */
watch([asset, sellAsset, buyAsset], () => {
  const picked = [asset.value, sellAsset.value, buyAsset.value].find(
    (ref) => ref?.chain && ref.chain !== 'fiat',
  )
  if (picked?.chain) network.value = picked.chain
})

function reset() {
  amount.value = ''
  priceUsd.value = ''
  fee.value = ''
  comment.value = ''
  txHash.value = ''
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

const save = useMutation({
  mutationFn: async () => {
    const cat = category.value.trim() || null
    const feeAmt = fee.value.trim() || null
    const hash = txHash.value.trim() || null
    const net = network.value || null

    if (isTrade.value) {
      if (!sellAsset.value || !buyAsset.value) throw new Error('asset')
      return call<Transaction[]>('add_trade', {
        input: {
          store_id: props.storeId,
          sell_symbol: sellAsset.value.symbol,
          sell_name: sellAsset.value.name,
          sell_coingecko_id: sellAsset.value.coingecko_id,
          sell_chain: sellAsset.value.chain,
          sell_contract: sellAsset.value.contract,
          sell_qty: amount.value.trim(),
          buy_symbol: buyAsset.value.symbol,
          buy_name: buyAsset.value.name,
          buy_coingecko_id: buyAsset.value.coingecko_id,
          buy_chain: buyAsset.value.chain,
          buy_contract: buyAsset.value.contract,
          buy_qty: priceUsd.value.trim(),
          comment: comment.value.trim(),
          category: cat,
          fee: feeAmt,
        },
      })
    }

    if (!asset.value) throw new Error('asset')
    const shared = {
      symbol: asset.value.symbol,
      name: asset.value.name,
      coingecko_id: asset.value.coingecko_id,
      chain: asset.value.chain,
      contract: asset.value.contract,
    }

    if (isTransfer.value) {
      return call<Transaction[]>('add_transfer', {
        input: {
          from_store_id: props.storeId,
          to_store_id: toStoreId.value,
          ...shared,
          amount: amount.value.trim(),
          comment: comment.value.trim(),
          category: cat,
          network: net,
          tx_hash: hash,
          fee: feeAmt,
        },
      })
    }

    return call<Transaction>('add_transaction', {
      input: {
        kind: mode.value === 'expense' ? 'expense' : 'income',
        ...shared,
        amount: amount.value.trim(),
        store_id: props.storeId,
        comment: comment.value.trim(),
        category: cat,
        timestamp: timestamp.value ? new Date(timestamp.value).toISOString() : null,
        tx_hash: hash,
        network: net,
        price_usd: priceUsd.value.trim() || null,
        fee: feeAmt,
      },
    })
  },
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
function prefill(plan: { mode: CalcMode; from: AssetRef | null; to: AssetRef | null; amount: string; price: string }) {
  if (plan.mode === 'converter' && plan.from && plan.to) {
    mode.value = 'trade'
    sellAsset.value = plan.from
    buyAsset.value = plan.to
    amount.value = plan.amount
    priceUsd.value = plan.price
  } else if (plan.from) {
    mode.value = 'income'
    asset.value = plan.from
    amount.value = plan.amount
    priceUsd.value = plan.price
  }
  comment.value = ''
}

defineExpose({ prefill })

const valid = computed(() => {
  if (!amount.value.trim()) return false
  if (isTrade.value) return Boolean(sellAsset.value && buyAsset.value && priceUsd.value.trim())
  if (!asset.value) return false
  if (isTransfer.value) return Boolean(toStoreId.value)
  return true
})

const modes: FormMode[] = ['income', 'expense', 'transfer', 'trade']
</script>

<template>
  <Card>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="font-medium">{{ t('form.title') }}</p>
        <div class="flex flex-wrap rounded-md border border-border p-0.5">
          <button
            v-for="entry in modes"
            :key="entry"
            type="button"
            class="rounded px-2.5 py-1 text-xs font-medium transition-colors"
            :class="mode === entry ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground'"
            @click="mode = entry"
          >
            {{ t(`form.mode.${entry}`) }}
          </button>
        </div>
      </div>
    </template>

    <form class="grid gap-4" @submit.prevent="save.mutate()">
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
        <div class="grid gap-1.5">
          <Label>{{ isTrade ? t('form.buyQty') : t('form.priceUsd') }}</Label>
          <Input
            v-model="priceUsd"
            inputmode="decimal"
            :placeholder="isTrade ? t('form.buyQtyHint') : t('form.priceHint')"
            :required="isTrade"
          />
        </div>
        <div class="grid gap-1.5">
          <Label>{{ t('form.fee') }}</Label>
          <Input v-model="fee" inputmode="decimal" placeholder="0" />
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

      <div class="grid gap-3 @sm:grid-cols-2 @lg:grid-cols-4">
        <div class="grid gap-1.5">
          <Label>{{ t('form.category') }}</Label>
          <Select v-model="category">
            <option value="">{{ t('form.noCategory') }}</option>
            <option
              v-for="entry in categories.data.value ?? []"
              :key="entry"
              :value="entry"
            >
              {{ entry }}
            </option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label>{{ t('form.network') }}</Label>
          <Select v-model="network">
            <option value="">{{ t('form.noNetwork') }}</option>
            <option v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</option>
          </Select>
        </div>
        <div v-if="!isTrade" class="grid gap-1.5">
          <Label>{{ t('form.date') }}</Label>
          <Input v-model="timestamp" type="datetime-local" />
        </div>
        <div class="grid gap-1.5">
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

      <div class="flex flex-wrap items-center gap-3">
        <Button type="submit" :disabled="!valid || save.isPending.value || props.disabled">
          {{ t('form.submit') }}
        </Button>
              </div>
    </form>
  </Card>
</template>
